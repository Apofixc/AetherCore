<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  BaseCard,
  AppButton,
  StatusBadge,
  SearchInput,
  BaseSelect,
  BaseModal
} from '@/components/common'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()

interface SessionItem {
  id: string
  username: string
  role: string
  ip: string
  time: string
  isCurrent: boolean
}

interface LogEntry {
  id: string
  timestamp: string
  level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG'
  source: string
  message: string
}

// Active sessions state
const sessions = ref<SessionItem[]>([
  {
    id: 'sess-1',
    username: 'root',
    role: 'Superuser',
    ip: '127.0.0.1',
    time: '11:55 PM',
    isCurrent: true
  },
  {
    id: 'sess-2',
    username: 's.jenkins',
    role: 'Administrator',
    ip: '192.168.1.45',
    time: '10:14 PM',
    isCurrent: false
  },
  {
    id: 'sess-3',
    username: 'm.vance',
    role: 'Operator',
    ip: '192.168.1.112',
    time: '08:30 PM',
    isCurrent: false
  }
])

// System logs state
const selectedLogFile = ref('[system] backend.log (3.5 MB)')
const selectedLogLevel = ref('ALL')
const logSearchQuery = ref('')
const isAutoRefresh = ref(true)
const fileInputRef = ref<HTMLInputElement | null>(null)
const showServiceStatusModal = ref(false)
const notificationMessage = ref('')
let refreshTimer: number | null = null

const logs = ref<LogEntry[]>([
  { id: '1', timestamp: '2026-08-20 20:14:51', level: 'INFO', source: 'nms.scheduler', message: 'AsyncScheduler stopped.' },
  { id: '2', timestamp: '2026-08-20 20:14:51', level: 'INFO', source: 'nms.plugin.loader', message: 'Loaded 4 WASM core dynamic modules.' },
  { id: '3', timestamp: '2026-08-20 20:15:00', level: 'INFO', source: 'nms.scheduler', message: 'AsyncScheduler started.' },
  { id: '4', timestamp: '2026-08-20 20:16:33', level: 'INFO', source: 'nms.messagebus', message: 'AetherCore Message Bus active on ipc://aethercore-bus' },
  { id: '5', timestamp: '2026-08-20 20:18:44', level: 'WARN', source: 'nms.plugin.loader', message: "Module 'legacy-auth' is deprecated and will be removed in v2.0." },
  { id: '6', timestamp: '2026-08-20 20:19:06', level: 'INFO', source: 'nms.auth.session', message: 'Operator session established for user [root] from 127.0.0.1' },
  { id: '7', timestamp: '2026-08-20 20:20:22', level: 'INFO', source: 'nms.db.pool', message: 'SQLite database connection pool verified: 0 pending locks.' },
  { id: '8', timestamp: '2026-08-20 20:22:41', level: 'INFO', source: 'nms.scheduler', message: 'Periodic telemetry broadcast completed (latency: 1.2ms).' }
])

const filteredLogs = computed(() => {
  return logs.value.filter((entry) => {
    // Level filter
    if (selectedLogLevel.value !== 'ALL' && entry.level !== selectedLogLevel.value) {
      return false
    }
    // Search query filter
    if (logSearchQuery.value.trim()) {
      const q = logSearchQuery.value.toLowerCase().trim()
      const text = `${entry.timestamp} ${entry.level} ${entry.source} ${entry.message}`.toLowerCase()
      if (!text.includes(q)) return false
    }
    return true
  })
})

function notify(msg: string) {
  notificationMessage.value = msg
  setTimeout(() => {
    notificationMessage.value = ''
  }, 3000)
}

function handleDownloadBackup() {
  const backupData = JSON.stringify({
    schema_version: '1.0.4',
    timestamp: new Date().toISOString(),
    system: 'AetherCore NMS Next-Gen',
    tables: ['users', 'roles', 'permissions', 'modules', 'audit_logs', 'security_settings']
  }, null, 2)

  const blob = new Blob([backupData], { type: 'application/octet-stream' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `nms_backup_${new Date().toISOString().slice(0, 10)}.db`
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  notify('Резервная копия nms.db успешно скачана')
}

function triggerRestoreFile() {
  fileInputRef.value?.click()
}

function handleFileSelected(e: Event) {
  const target = e.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    const file = target.files[0]
    notify(`Файл базы данных ${file.name} успешно проверен и восстановлен`)
    target.value = ''
  }
}

function handleRotateAudit() {
  logs.value.unshift({
    id: String(Date.now()),
    timestamp: new Date().toISOString().replace('T', ' ').slice(0, 19),
    level: 'INFO',
    source: 'nms.audit.rotator',
    message: 'Audit log rotated: active log archived to audit_archive_2026_08.db'
  })
  notify('Журнал аудита успешно ротирован')
}

function revokeSession(id: string) {
  sessions.value = sessions.value.filter((s) => s.id !== id)
  notify('Сессия успешно отозвана')
}

function terminateOthers() {
  sessions.value = sessions.value.filter((s) => s.isCurrent)
  notify('Все сторонние сессии успешно завершены')
}

function handleAllLogout() {
  authStore.logout()
  router.push('/login')
}

function clearConsole() {
  logs.value = []
  notify('Экран консоли логов очищен')
}

function refreshLogs() {
  const now = new Date().toISOString().replace('T', ' ').slice(0, 19)
  logs.value.unshift({
    id: String(Date.now()),
    timestamp: now,
    level: 'INFO',
    source: 'nms.system',
    message: 'System log stream synchronized successfully.'
  })
}

function downloadCurrentLog() {
  const content = logs.value
    .map((l) => `${l.timestamp} | ${l.level.padEnd(5)} | ${l.source} | ${l.message}`)
    .join('\n')
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `system_${selectedLogFile.value.replace(/[^a-zA-Z0-9]/g, '_')}.log`
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  notify('Файл логов скачан')
}

const logFileOptions = [
  '[system] backend.log (3.5 MB)',
  '[system] error.log (1.2 MB)',
  '[auth] access.log (0.8 MB)',
  '[database] query.log (12.4 MB)'
]

const logLevelOptions = [
  { value: 'ALL', label: 'ALL' },
  { value: 'ERROR', label: 'ERROR' },
  { value: 'WARN', label: 'WARN' },
  { value: 'INFO', label: 'INFO' },
  { value: 'DEBUG', label: 'DEBUG' }
]

onMounted(() => {
  refreshTimer = window.setInterval(() => {
    if (isAutoRefresh.value && logs.value.length < 200) {
      const now = new Date().toISOString().replace('T', ' ').slice(0, 19)
      const mockEvents = [
        { level: 'INFO' as const, source: 'nms.scheduler', message: 'Periodic telemetry heartbeat: all systems nominal' },
        { level: 'INFO' as const, source: 'nms.messagebus', message: 'IPC message dispatched to 4 listener nodes' },
        { level: 'DEBUG' as const, source: 'nms.kv.store', message: 'KV cache cleanup: 0 expired keys evicted' }
      ]
      const randomEvent = mockEvents[Math.floor(Math.random() * mockEvents.length)]
      logs.value.push({
        id: String(Date.now()),
        timestamp: now,
        level: randomEvent.level,
        source: randomEvent.source,
        message: randomEvent.message
      })
    }
  }, 3000)
})

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer)
})
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">
        <!-- Toast Notification Banner -->
        <div
          v-if="notificationMessage"
          class="fixed bottom-12 right-6 z-50 bg-surface-container-high border border-primary-fixed-dim/50 text-primary-fixed-dim px-4 py-2 rounded-xl shadow-glow-primary-md flex items-center gap-2 text-xs font-bold font-mono animate-fade-in"
        >
          <span class="material-symbols-outlined text-sm">check_circle</span>
          <span>{{ notificationMessage }}</span>
        </div>

        <!-- Top Page Header -->
        <PageHeader
          :title="t('system.title')"
          :subtitle="t('system.subtitle')"
          icon="admin_panel_settings"
        />

        <!-- Top Row: Backup & Restore + Active Sessions -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-lg">
          <!-- Card 1: Backup & Restore -->
          <BaseCard
            :title="t('system.backupRestoreTitle')"
            :subtitle="t('system.backupDesc')"
            icon="cloud_sync"
          >
            <input
              ref="fileInputRef"
              type="file"
              accept=".db,.sqlite,.sqlite3"
              class="hidden"
              @change="handleFileSelected"
            />

            <div class="flex flex-wrap gap-sm pt-2">
              <AppButton
                variant="primary"
                size="sm"
                icon="download"
                @click="handleDownloadBackup"
              >
                {{ t('system.downloadBackup') }}
              </AppButton>
              <AppButton
                variant="outline"
                size="sm"
                icon="upload_file"
                @click="triggerRestoreFile"
              >
                {{ t('system.restoreFromFile') }}
              </AppButton>
              <AppButton
                variant="outline"
                size="sm"
                icon="history"
                @click="handleRotateAudit"
              >
                {{ t('system.rotateAudit') }}
              </AppButton>
            </div>
          </BaseCard>

          <!-- Card 2: Active Sessions -->
          <BaseCard
            :title="t('system.activeSessionsTitle')"
            :subtitle="t('system.activeSessionsDesc')"
            icon="hub"
          >
            <template #headerActions>
              <AppButton
                variant="danger"
                size="xs"
                icon="security"
                @click="terminateOthers"
              >
                {{ t('system.terminateOthers') }}
              </AppButton>
              <AppButton
                variant="outline"
                size="xs"
                icon="logout"
                @click="handleAllLogout"
              >
                {{ t('system.allLogout') }}
              </AppButton>
            </template>

            <!-- Sessions List -->
            <div class="flex flex-col gap-2 max-h-48 overflow-y-auto pr-1">
              <div
                v-for="s in sessions"
                :key="s.id"
                class="flex items-center justify-between p-2.5 bg-surface-container-highest/50 rounded-xl border border-outline-variant/40"
              >
                <div class="flex items-center gap-2.5 flex-wrap">
                  <StatusBadge
                    :variant="s.isCurrent ? 'success' : 'neutral'"
                    :pulse="s.isCurrent"
                    :dot="true"
                    size="xs"
                  >
                    {{ s.isCurrent ? t('system.currentSession') : s.username }}
                  </StatusBadge>
                  <span v-if="s.isCurrent" class="font-mono text-xs font-bold text-on-surface">{{ s.username }}</span>
                  <span class="text-[11px] text-on-surface-variant font-mono">
                    ({{ s.role }}) [{{ s.ip }}]
                  </span>
                </div>
                <div class="flex items-center gap-3">
                  <span class="text-[10px] font-mono text-on-surface-variant">{{ s.time }}</span>
                  <AppButton
                    v-if="!s.isCurrent"
                    variant="danger"
                    size="xs"
                    @click="revokeSession(s.id)"
                  >
                    {{ t('system.revokeSession') }}
                  </AppButton>
                </div>
              </div>
            </div>
          </BaseCard>
        </div>

        <!-- Card 3: System Logs Viewer -->
        <BaseCard
          :title="t('system.systemLogsViewerTitle')"
          :subtitle="t('system.systemLogsViewerSubtitle')"
          icon="terminal"
          :no-padding="true"
        >
          <template #headerActions>
            <!-- Log File Selector -->
            <div class="w-48">
              <BaseSelect
                v-model="selectedLogFile"
                :options="logFileOptions"
                size="sm"
              />
            </div>

            <!-- Log Level Selector -->
            <div class="w-28">
              <BaseSelect
                v-model="selectedLogLevel"
                :options="logLevelOptions"
                size="sm"
              />
            </div>

            <!-- Search Input -->
            <SearchInput
              v-model="logSearchQuery"
              :placeholder="t('system.searchInLogs')"
              width-class="w-48"
            />

            <!-- Auto-refresh Checkbox -->
            <label class="flex items-center gap-1.5 cursor-pointer ml-1">
              <input
                v-model="isAutoRefresh"
                type="checkbox"
                class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0 cursor-pointer"
              />
              <span class="text-[10px] text-on-surface-variant select-none">{{ t('system.autoRefresh') }}</span>
            </label>

            <!-- Action Icons -->
            <div class="flex items-center gap-1">
              <AppButton
                variant="outline"
                size="xs"
                icon="refresh"
                :title="t('system.refreshLogs')"
                @click="refreshLogs"
              />
              <AppButton
                variant="outline"
                size="xs"
                icon="cleaning_services"
                :title="t('system.clearLogs')"
                @click="clearConsole"
              />
              <AppButton
                variant="outline"
                size="xs"
                icon="dns"
                :title="t('system.serviceStatus')"
                @click="showServiceStatusModal = true"
              />
              <AppButton
                variant="outline"
                size="xs"
                icon="download"
                :title="t('system.downloadLogs')"
                @click="downloadCurrentLog"
              />
            </div>
          </template>

          <!-- Console Terminal Output Area -->
          <div class="bg-surface-deep p-md font-mono text-xs h-[420px] overflow-y-auto flex flex-col gap-1 select-text">
            <div
              v-for="entry in filteredLogs"
              :key="entry.id"
              class="flex items-start gap-3 py-0.5 hover:bg-surface-container/40 px-1.5 rounded transition-colors"
            >
              <span class="text-on-surface-variant shrink-0 text-[11px] select-none">
                {{ entry.timestamp }} | {{ entry.level.padEnd(5) }}
              </span>
              <span class="text-outline-variant select-none">|</span>
              <span
                class="font-semibold select-none shrink-0"
                :class="entry.level === 'ERROR' ? 'text-error' : entry.level === 'WARN' ? 'text-warning-yellow' : 'text-primary-fixed-dim'"
              >
                {{ entry.source }}
              </span>
              <span class="text-outline-variant select-none">|</span>
              <span
                class="break-all"
                :class="entry.level === 'ERROR' ? 'text-error font-bold' : entry.level === 'WARN' ? 'text-warning-yellow' : 'text-tertiary-fixed-dim'"
              >
                {{ entry.message }}
              </span>
            </div>

            <div v-if="filteredLogs.length === 0" class="text-center py-12 text-on-surface-variant/60 text-xs">
              Нет логов, соответствующих выбранным фильтрам
            </div>
          </div>
        </BaseCard>
      </div>
    </main>

    <!-- Modal: Service Status Dialog -->
    <BaseModal
      v-model="showServiceStatusModal"
      title="Статус сервисов AetherCore NMS"
      icon="dns"
      max-width="max-w-md"
    >
      <div class="flex flex-col gap-2.5 text-xs font-body-mono">
        <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
          <span>AetherCore Core Daemon</span>
          <StatusBadge variant="success" size="xs">RUNNING (pid 4182)</StatusBadge>
        </div>
        <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
          <span>WASM Plugin Runtime</span>
          <StatusBadge variant="success" size="xs">READY (4 active)</StatusBadge>
        </div>
        <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
          <span>IPC Message Bus</span>
          <StatusBadge variant="success" size="xs">CONNECTED</StatusBadge>
        </div>
        <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
          <span>SQLite Embedded DB</span>
          <StatusBadge variant="success" size="xs">SYNCED (WAL mode)</StatusBadge>
        </div>
      </div>

      <template #footer>
        <AppButton
          variant="primary"
          size="sm"
          @click="showServiceStatusModal = false"
        >
          Закрыть
        </AppButton>
      </template>
    </BaseModal>
  </div>
</template>
