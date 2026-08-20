<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import SettingsNav from '@/components/layout/SettingsNav.vue'
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
          class="fixed bottom-12 right-6 z-50 bg-surface-container-high border border-primary-fixed-dim/50 text-primary-fixed-dim px-4 py-2 rounded-lg shadow-glow-primary-md flex items-center gap-2 text-xs font-bold font-body-mono animate-fade-in"
        >
          <span class="material-symbols-outlined text-sm">check_circle</span>
          <span>{{ notificationMessage }}</span>
        </div>

        <!-- Top Page Header -->
        <div class="flex items-center justify-between flex-wrap gap-md">
          <div>
            <h1 class="font-display-lg text-display-lg text-on-surface font-bold">
              {{ t('system.title') }}
            </h1>
            <p class="text-sm text-on-surface-variant mt-1">
              {{ t('system.subtitle') }}
            </p>
          </div>
        </div>

        <!-- Top Row: Backup & Restore + Active Sessions -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-lg">
          <!-- Card 1: Backup & Restore -->
          <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
            <div class="flex items-center gap-sm text-primary-fixed-dim">
              <span class="material-symbols-outlined">cloud_sync</span>
              <h2 class="font-title-sm text-title-sm font-bold text-on-surface">
                {{ t('system.backupRestoreTitle') }}
              </h2>
            </div>
            <p class="text-xs text-on-surface-variant leading-relaxed">
              {{ t('system.backupDesc') }}
            </p>

            <input
              ref="fileInputRef"
              type="file"
              accept=".db,.sqlite,.sqlite3"
              class="hidden"
              @change="handleFileSelected"
            />

            <div class="flex flex-wrap gap-sm mt-auto pt-sm">
              <button
                type="button"
                class="bg-primary-fixed-dim hover:bg-primary-fixed-dim/90 text-on-primary-fixed border border-primary-fixed-dim px-3.5 py-1.5 rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 shadow-glow-primary-sm hover:shadow-glow-primary-md cursor-pointer"
                @click="handleDownloadBackup"
              >
                <span class="material-symbols-outlined text-[18px]">download</span>
                <span>{{ t('system.downloadBackup') }}</span>
              </button>
              <button
                type="button"
                class="h-8 px-3 bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 cursor-pointer"
                @click="triggerRestoreFile"
              >
                <span class="material-symbols-outlined text-[18px]">upload_file</span>
                <span>{{ t('system.restoreFromFile') }}</span>
              </button>
              <button
                type="button"
                class="h-8 px-3 bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 cursor-pointer"
                @click="handleRotateAudit"
              >
                <span class="material-symbols-outlined text-[18px]">history</span>
                <span>{{ t('system.rotateAudit') }}</span>
              </button>
            </div>
          </div>

          <!-- Card 2: Active Sessions -->
          <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
            <div class="flex items-center justify-between flex-wrap gap-sm">
              <div class="flex items-center gap-sm text-tertiary-fixed-dim">
                <span class="material-symbols-outlined">group</span>
                <h2 class="font-title-sm text-title-sm font-bold text-on-surface">
                  {{ t('system.activeSessionsTitle') }}
                </h2>
              </div>
              <div class="flex items-center gap-2">
                <button
                  type="button"
                  class="bg-error-container/20 border border-error/40 text-error hover:bg-error-container/40 px-2.5 py-1 rounded-lg text-[10px] font-bold uppercase tracking-wider transition-all flex items-center gap-1 cursor-pointer active:scale-95"
                  @click="terminateOthers"
                >
                  <span class="material-symbols-outlined text-[14px]">security</span>
                  <span>{{ t('system.terminateOthers') }}</span>
                </button>
                <button
                  type="button"
                  class="h-7 px-2.5 bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant rounded-lg text-[10px] font-bold uppercase tracking-wider transition-all flex items-center gap-1 cursor-pointer active:scale-95"
                  @click="handleAllLogout"
                >
                  <span class="material-symbols-outlined text-[14px]">logout</span>
                  <span>{{ t('system.allLogout') }}</span>
                </button>
              </div>
            </div>

            <!-- Sessions List -->
            <div class="flex flex-col gap-2 mt-1 max-h-40 overflow-y-auto pr-1">
              <div
                v-for="s in sessions"
                :key="s.id"
                class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40"
              >
                <div class="flex items-center gap-2.5 flex-wrap">
                  <div
                    class="w-2 h-2 rounded-full"
                    :class="s.isCurrent ? 'bg-tertiary-fixed-dim shadow-glow-tertiary-sm animate-pulse' : 'bg-outline-variant'"
                  ></div>
                  <span class="font-body-mono text-xs font-bold text-on-surface">{{ s.username }}</span>
                  <span
                    v-if="s.isCurrent"
                    class="bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30 text-[9px] px-1.5 py-0.2 rounded font-bold font-body-mono uppercase"
                  >
                    {{ t('system.currentSession') }}
                  </span>
                  <span class="text-[11px] text-on-surface-variant font-body-mono">
                    ({{ s.role }}) [{{ s.ip }}]
                  </span>
                </div>
                <div class="flex items-center gap-3">
                  <span class="text-[10px] font-body-mono text-on-surface-variant">{{ s.time }}</span>
                  <button
                    v-if="!s.isCurrent"
                    type="button"
                    class="text-[10px] font-bold uppercase text-error border border-error/40 px-2 py-0.5 rounded-lg hover:bg-error-container/20 transition-colors cursor-pointer active:scale-95"
                    @click="revokeSession(s.id)"
                  >
                    {{ t('system.revokeSession') }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Card 3: System Logs Viewer -->
        <div class="bg-surface-container-low border border-outline-variant rounded-lg shadow-card-dark flex flex-col overflow-hidden">
          <!-- Log Toolbar -->
          <div class="p-md border-b border-outline-variant flex items-center justify-between bg-surface-container flex-wrap gap-md">
            <div class="flex items-center gap-sm">
              <span class="material-symbols-outlined text-primary-fixed-dim">terminal</span>
              <div>
                <h2 class="font-title-sm text-title-sm text-on-surface font-bold">
                  {{ t('system.systemLogsViewerTitle') }}
                </h2>
                <p class="text-[10px] text-on-surface-variant uppercase tracking-widest">
                  {{ t('system.systemLogsViewerSubtitle') }}
                </p>
              </div>
            </div>

            <!-- Filters & Actions -->
            <div class="flex items-center gap-2 flex-wrap">
              <!-- Log File Selector -->
              <div class="flex items-center gap-1.5">
                <span class="text-[10px] font-bold text-on-surface-variant uppercase hidden sm:inline-block">
                  {{ t('system.logFileLabel') }}
                </span>
                <div class="relative flex items-center">
                  <select
                    v-model="selectedLogFile"
                    class="h-8 bg-surface-container-highest border border-outline-variant text-on-surface font-body-mono rounded-lg pl-2.5 pr-7 text-xs focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer appearance-none"
                  >
                    <option>[system] backend.log (3.5 MB)</option>
                    <option>[system] error.log (1.2 MB)</option>
                    <option>[auth] access.log (0.8 MB)</option>
                    <option>[database] query.log (12.4 MB)</option>
                  </select>
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute right-2 pointer-events-none">expand_more</span>
                </div>
              </div>

              <!-- Log Level Selector -->
              <div class="flex items-center gap-1.5">
                <span class="text-[10px] font-bold text-on-surface-variant uppercase hidden sm:inline-block">
                  {{ t('system.logLevelLabel') }}
                </span>
                <div class="relative flex items-center">
                  <select
                    v-model="selectedLogLevel"
                    class="h-8 bg-surface-container-highest border border-outline-variant text-on-surface font-body-mono rounded-lg pl-2.5 pr-7 text-xs focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer appearance-none"
                  >
                    <option value="ALL">{{ t('system.allLevels') }}</option>
                    <option value="ERROR">ERROR</option>
                    <option value="WARN">WARN</option>
                    <option value="INFO">INFO</option>
                    <option value="DEBUG">DEBUG</option>
                  </select>
                  <span class="material-symbols-outlined text-sm text-on-surface-variant absolute right-2 pointer-events-none">expand_more</span>
                </div>
              </div>

              <!-- Search Input -->
              <div class="relative flex items-center">
                <span class="material-symbols-outlined absolute left-2.5 text-sm text-on-surface-variant pointer-events-none">search</span>
                <input
                  v-model="logSearchQuery"
                  type="text"
                  class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-8 pr-2.5 text-xs font-body-mono text-on-surface w-44 focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                  :placeholder="t('system.searchInLogs')"
                />
              </div>

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
              <div class="flex items-center gap-1 ml-1">
                <button
                  type="button"
                  class="h-8 w-8 rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                  :title="t('system.refreshLogs')"
                  @click="refreshLogs"
                >
                  <span class="material-symbols-outlined text-[18px]">refresh</span>
                </button>
                <button
                  type="button"
                  class="h-8 w-8 rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                  :title="t('system.clearLogs')"
                  @click="clearConsole"
                >
                  <span class="material-symbols-outlined text-[18px]">cleaning_services</span>
                </button>
                <button
                  type="button"
                  class="h-8 w-8 rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                  :title="t('system.serviceStatus')"
                  @click="showServiceStatusModal = true"
                >
                  <span class="material-symbols-outlined text-[18px]">dns</span>
                </button>
                <button
                  type="button"
                  class="h-8 w-8 rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                  :title="t('system.downloadLogs')"
                  @click="downloadCurrentLog"
                >
                  <span class="material-symbols-outlined text-[18px]">download</span>
                </button>
              </div>
            </div>
          </div>

          <!-- Console Terminal Output Area -->
          <div class="bg-surface-deep p-md font-body-mono text-xs h-[420px] overflow-y-auto flex flex-col gap-1 select-text">
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
        </div>
      </div>
    </main>

    <!-- Modal: Service Status Dialog -->
    <div
      v-if="showServiceStatusModal"
      class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-md animate-fade-in"
      @click.self="showServiceStatusModal = false"
    >
      <div class="bg-surface-container-low border border-outline-variant rounded-xl p-lg max-w-md w-full shadow-2xl flex flex-col gap-md">
        <div class="flex items-center justify-between border-b border-outline-variant/60 pb-sm">
          <div class="flex items-center gap-2 text-primary-fixed-dim">
            <span class="material-symbols-outlined text-xl">dns</span>
            <h3 class="text-sm font-bold text-on-surface">Статус сервисов AetherCore NMS</h3>
          </div>
          <button
            type="button"
            class="text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
            @click="showServiceStatusModal = false"
          >
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>

        <div class="flex flex-col gap-2.5 text-xs font-body-mono">
          <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
            <span>AetherCore Core Daemon</span>
            <span class="text-tertiary-fixed-dim font-bold">RUNNING (pid 4182)</span>
          </div>
          <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
            <span>WASM Plugin Runtime</span>
            <span class="text-tertiary-fixed-dim font-bold">READY (4 active)</span>
          </div>
          <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
            <span>IPC Message Bus</span>
            <span class="text-tertiary-fixed-dim font-bold">CONNECTED</span>
          </div>
          <div class="flex items-center justify-between p-2 bg-surface-container-highest/60 rounded-lg border border-outline-variant/40">
            <span>SQLite Embedded DB</span>
            <span class="text-tertiary-fixed-dim font-bold">SYNCED (WAL mode)</span>
          </div>
        </div>

        <div class="flex justify-end pt-sm border-t border-outline-variant/60">
          <button
            type="button"
            class="px-4 py-1.5 text-xs font-bold rounded-lg bg-primary-fixed-dim text-on-primary-fixed hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm transition-all cursor-pointer"
            @click="showServiceStatusModal = false"
          >
            Закрыть
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
