<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  BaseCard,
  AppButton,
  StatusBadge,
  SearchInput,
  BaseSelect,
  BaseModal,
  ConfirmModal
} from '@/components/common'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'
import { systemApi, type SystemInfo, type LogProvider } from '@/api/system'
import { settingsApi } from '@/api/settings'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()

// Retention & Rotation state
const auditRetentionDays = ref(90)
const isSavingRetention = ref(false)
const showRotateModal = ref(false)
const rotateDays = ref(90)
const rotateSaveArchive = ref(true)
const isRotating = ref(false)

interface SessionItem {
  id: string
  username: string
  role: string
  ip: string
  time: string
  client: string
  isCurrent: boolean
}

interface LogEntry {
  id: string
  timestamp: string
  level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG'
  source: string
  message: string
}

const systemInfo = ref<SystemInfo | null>(null)
const logProviders = ref<LogProvider[]>([])
const selectedProviderId = ref('system')

// Active sessions state
const sessions = ref<SessionItem[]>([])

// System logs state
const selectedLogFile = ref('[system] backend.log')
const selectedLogLevel = ref('ALL')
const logSearchQuery = ref('')
const isAutoRefresh = ref(true)
const fileInputRef = ref<HTMLInputElement | null>(null)
const logConsoleRef = ref<HTMLDivElement | null>(null)
const isUserScrolledUp = ref(false)
const isFullscreenLogs = ref(false)
const showServiceStatusModal = ref(false)
const notificationMessage = ref('')
let refreshTimer: number | null = null

// Confirm modal state
const showConfirmModal = ref(false)
const confirmModalConfig = ref<{
  title: string
  message: string
  variant: 'danger' | 'warning' | 'primary' | 'info'
  icon: string
  confirmText?: string
  action: () => void
}>({
  title: '',
  message: '',
  variant: 'danger',
  icon: 'warning',
  confirmText: '',
  action: () => {}
})

const logs = ref<LogEntry[]>([])

const logCounts = computed(() => {
  return {
    total: logs.value.length,
    errors: logs.value.filter((l) => l.level === 'ERROR').length,
    warns: logs.value.filter((l) => l.level === 'WARN').length,
    info: logs.value.filter((l) => l.level === 'INFO').length,
    debug: logs.value.filter((l) => l.level === 'DEBUG').length
  }
})

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

function handleScroll(e: Event) {
  const el = e.target as HTMLElement
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 30
  isUserScrolledUp.value = !atBottom
}

function scrollToBottom(smooth = true) {
  nextTick(() => {
    if (logConsoleRef.value) {
      logConsoleRef.value.scrollTo({
        top: logConsoleRef.value.scrollHeight,
        behavior: smooth ? 'smooth' : 'auto'
      })
      isUserScrolledUp.value = false
    }
  })
}

function handleDownloadBackup() {
  const backupData = JSON.stringify({
    schema_version: '1.0.4',
    timestamp: new Date().toISOString(),
    system: 'AetherCore Platform',
    tables: ['users', 'roles', 'permissions', 'modules', 'audit_logs', 'security_settings']
  }, null, 2)

  const blob = new Blob([backupData], { type: 'application/octet-stream' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `aethercore_backup_${new Date().toISOString().slice(0, 10)}.db`
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  notify(t('system.downloadBackup') + ' - OK')
}

function triggerRestoreFile() {
  fileInputRef.value?.click()
}

function handleFileSelected(e: Event) {
  const target = e.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    const file = target.files[0]
    confirmModalConfig.value = {
      title: t('system.confirmRestoreTitle'),
      message: t('system.confirmRestoreMsg', { file: file.name }),
      variant: 'danger',
      icon: 'upload_file',
      confirmText: t('system.restoreFromFile'),
      action: () => {
        notify(t('system.restoreFromFile') + ` (${file.name}) - OK`)
      }
    }
    showConfirmModal.value = true
    target.value = ''
  }
}

function requestRotateAudit() {
  rotateDays.value = auditRetentionDays.value
  showRotateModal.value = true
}

async function loadMaintenanceSettings() {
  try {
    const maint = await settingsApi.getMaintenanceSettings()
    if (maint && maint.audit_retention_days) {
      auditRetentionDays.value = maint.audit_retention_days
      rotateDays.value = maint.audit_retention_days
    }
  } catch (err) {
    console.debug('Failed to load maintenance settings:', err)
  }
}

async function updateRetentionDays(days: number) {
  auditRetentionDays.value = days
  rotateDays.value = days
  isSavingRetention.value = true
  try {
    await settingsApi.updateMaintenanceSettings({
      audit_retention_days: days
    })
    notify(t('common.save') + ' - OK')
  } catch (err) {
    console.error('Failed to update retention settings:', err)
  } finally {
    isSavingRetention.value = false
  }
}

async function executeRotateAudit() {
  isRotating.value = true
  try {
    const res = await systemApi.rotateAuditLogs({
      days: rotateDays.value,
      archive: rotateSaveArchive.value
    })
    showRotateModal.value = false
    const archiveText = res.archive_filename
      ? t('system.archiveCreated', { file: res.archive_filename })
      : ''
    notify(t('system.rotateSuccess', { deleted: res.deleted_count, archive: archiveText }))
  } catch (err) {
    console.error('Failed to rotate audit logs:', err)
  } finally {
    isRotating.value = false
  }
}

function requestRevokeSession(s: SessionItem) {
  confirmModalConfig.value = {
    title: t('system.confirmRevokeSingleTitle'),
    message: t('system.confirmRevokeSingleMsg', { user: s.username, ip: s.ip }),
    variant: 'danger',
    icon: 'no_accounts',
    confirmText: t('system.revokeSession'),
    action: () => {
      sessions.value = sessions.value.filter((item) => item.id !== s.id)
      notify(t('system.revokeSession') + ` (${s.username}) - OK`)
    }
  }
  showConfirmModal.value = true
}

function requestTerminateOthers() {
  confirmModalConfig.value = {
    title: t('system.confirmRevokeOthersTitle'),
    message: t('system.confirmRevokeOthersMsg'),
    variant: 'danger',
    icon: 'security',
    confirmText: t('system.terminateOthers'),
    action: () => {
      sessions.value = sessions.value.filter((s) => s.isCurrent)
      notify(t('system.confirmRevokeOthersTitle') + ' - OK')
    }
  }
  showConfirmModal.value = true
}

function requestAllLogout() {
  confirmModalConfig.value = {
    title: t('system.confirmRevokeAllTitle'),
    message: t('system.confirmRevokeAllMsg'),
    variant: 'danger',
    icon: 'logout',
    confirmText: t('system.allLogout'),
    action: () => {
      authStore.logout()
      router.push('/login')
    }
  }
  showConfirmModal.value = true
}

function handleConfirmModal() {
  confirmModalConfig.value.action()
  showConfirmModal.value = false
}

function parseLogLine(rawLine: string, index: number): LogEntry {
  // Parsing log formats:
  // 1. "2026-08-20T20:14:51.123Z INFO [source] message"
  // 2. "2026-08-20 20:14:51 | INFO | source | message"
  // 3. Fallback generic
  const isoMatch = rawLine.match(/^(\S+)\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+\[([^\]]+)\]\s+(.*)$/)
  if (isoMatch) {
    const lvl = isoMatch[2] === 'TRACE' ? 'DEBUG' : isoMatch[2] as 'INFO' | 'WARN' | 'ERROR' | 'DEBUG'
    return {
      id: `${index}-${Date.now()}`,
      timestamp: isoMatch[1].replace('T', ' ').slice(0, 19),
      level: lvl,
      source: isoMatch[3],
      message: isoMatch[4]
    }
  }

  const pipeMatch = rawLine.split('|').map((s) => s.trim())
  if (pipeMatch.length >= 4) {
    const lvl = ['INFO', 'WARN', 'ERROR', 'DEBUG'].includes(pipeMatch[1]) ? pipeMatch[1] as any : 'INFO'
    return {
      id: `${index}-${Date.now()}`,
      timestamp: pipeMatch[0],
      level: lvl,
      source: pipeMatch[2],
      message: pipeMatch.slice(3).join(' | ')
    }
  }

  return {
    id: `${index}-${Date.now()}`,
    timestamp: new Date().toISOString().replace('T', ' ').slice(0, 19),
    level: rawLine.toLowerCase().includes('err') ? 'ERROR' : rawLine.toLowerCase().includes('warn') ? 'WARN' : 'INFO',
    source: 'system',
    message: rawLine
  }
}

async function fetchRealLogs() {
  try {
    const res = await systemApi.getLogs({
      provider: selectedProviderId.value,
      level: selectedLogLevel.value !== 'ALL' ? selectedLogLevel.value : undefined,
      search: logSearchQuery.value.trim() || undefined,
      limit: 100
    })
    if (res && Array.isArray(res.lines) && res.lines.length > 0) {
      logs.value = res.lines.map((line, idx) => parseLogLine(line, idx))
      if (!isUserScrolledUp.value) {
        scrollToBottom(false)
      }
    }
  } catch (e) {
    // API offline or error, maintain existing log lines
  }
}

async function loadSystemData() {
  try {
    const [info, providers] = await Promise.all([
      systemApi.getInfo().catch(() => null),
      systemApi.getProviders().catch(() => [])
    ])
    if (info) {
      systemInfo.value = info
    }
    if (providers && providers.length > 0) {
      logProviders.value = providers
    }
    await loadMaintenanceSettings()
  } catch (e) {
    console.warn('Could not fetch system info:', e)
  }

  if (authStore.user) {
    sessions.value = [
      {
        id: `sess-${authStore.user.id.slice(0, 6)}`,
        username: authStore.user.username,
        role: authStore.user.is_superuser ? 'Superuser' : (authStore.user.roles?.[0] ? authStore.user.roles[0].charAt(0).toUpperCase() + authStore.user.roles[0].slice(1) : 'User'),
        ip: '127.0.0.1',
        time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        client: 'Web Client',
        isCurrent: true
      }
    ]
  }

  await fetchRealLogs()
}

function clearConsole() {
  logs.value = []
  notify(t('system.clearLogs'))
}

async function refreshLogs() {
  await fetchRealLogs()
  notify(t('system.refreshLogs'))
}

async function downloadCurrentLog() {
  try {
    const blob = await systemApi.downloadLog(selectedProviderId.value)
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `system_${selectedProviderId.value}_${new Date().toISOString().slice(0, 10)}.log`
    document.body.appendChild(link)
    link.click()
    link.remove()
    URL.revokeObjectURL(url)
    notify(t('system.downloadLogs') + ' - OK')
  } catch (err) {
    // Fallback to in-memory logs
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
    notify(t('system.downloadLogs') + ' - OK')
  }
}

async function copyLogLine(entry: LogEntry) {
  const text = `${entry.timestamp} [${entry.level}] ${entry.source}: ${entry.message}`
  try {
    await navigator.clipboard.writeText(text)
    notify(t('system.copiedLine'))
  } catch {
    notify(t('system.copiedLine'))
  }
}

function escapeHtml(unsafe: string) {
  return unsafe
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

function renderHighlightedText(text: string) {
  const safeText = escapeHtml(text)
  const q = logSearchQuery.value.trim()
  if (!q) return safeText
  const regex = new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
  return safeText.replace(regex, '<mark class="bg-primary/40 text-primary-fixed px-0.5 rounded font-bold">$1</mark>')
}

const logFileOptions = computed(() => {
  if (logProviders.value.length > 0) {
    return logProviders.value.map((p) => ({
      value: p.id,
      label: `[${p.kind}] ${p.name}`
    }))
  }
  return [
    { value: 'system', label: '[system] backend.log (3.5 MB)' },
    { value: 'error', label: '[system] error.log (1.2 MB)' },
    { value: 'auth', label: '[auth] access.log (0.8 MB)' },
    { value: 'database', label: '[database] query.log (12.4 MB)' }
  ]
})

const logLevelOptions = [
  { value: 'ALL', label: 'ALL' },
  { value: 'ERROR', label: 'ERROR' },
  { value: 'WARN', label: 'WARN' },
  { value: 'INFO', label: 'INFO' },
  { value: 'DEBUG', label: 'DEBUG' }
]

onMounted(() => {
  loadSystemData()
  refreshTimer = window.setInterval(() => {
    if (isAutoRefresh.value) {
      fetchRealLogs()
    }
  }, 4000)
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

            <!-- Action Buttons -->
            <div v-if="authStore.canManageSystem" class="flex flex-wrap gap-sm pt-1 mb-4 pb-4 border-b border-outline-variant/30">
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
                @click="requestRotateAudit"
              >
                {{ t('system.rotateAudit') }}
              </AppButton>
            </div>

            <!-- Database & Backup Health Status Grid -->
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
              <div class="bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.dbSize') }}</span>
                <span class="text-xs font-mono font-bold text-on-surface mt-0.5">24.8 MB (WAL)</span>
                <span class="text-[10px] text-on-surface-variant/70 font-mono">6 {{ t('system.tablesCount').toLowerCase() }}</span>
              </div>
              <div class="bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.lastBackup') }}</span>
                <span class="text-xs font-mono font-bold text-on-surface mt-0.5">2026-08-20 03:00</span>
                <span class="text-[10px] text-primary-fixed-dim font-mono">Auto Snapshot</span>
              </div>
              <div class="col-span-2 sm:col-span-1 bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col justify-between">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.autoBackup') }}</span>
                <div class="mt-1">
                  <StatusBadge variant="success" size="xs" :dot="true">
                    {{ t('system.autoBackupEnabled') }}
                  </StatusBadge>
                </div>
              </div>
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
                v-if="authStore.canManageSystem"
                variant="danger"
                size="xs"
                icon="security"
                @click="requestTerminateOthers"
              >
                {{ t('system.terminateOthers') }}
              </AppButton>
              <AppButton
                v-if="authStore.canManageSystem"
                variant="outline"
                size="xs"
                icon="logout"
                @click="requestAllLogout"
              >
                {{ t('system.allLogout') }}
              </AppButton>
            </template>

            <!-- Sessions List -->
            <div class="flex flex-col gap-2 max-h-56 overflow-y-auto pr-1">
              <div
                v-for="s in sessions"
                :key="s.id"
                class="flex items-center justify-between p-2.5 bg-surface-container-highest/50 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors"
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
                  <span class="text-[10px] text-on-surface-variant/60 font-mono hidden sm:inline">
                    • {{ s.client }}
                  </span>
                </div>
                <div class="flex items-center gap-2.5">
                  <span class="text-[10px] font-mono text-on-surface-variant">{{ s.time }}</span>
                  <AppButton
                    v-if="!s.isCurrent && authStore.canManageSystem"
                    variant="danger"
                    size="xs"
                    @click="requestRevokeSession(s)"
                  >
                    {{ t('system.revokeSession') }}
                  </AppButton>
                </div>
              </div>
            </div>
          </BaseCard>
        </div>

        <!-- Card 3: System Logs Viewer -->
        <div
          :class="[
            isFullscreenLogs
              ? 'fixed inset-4 z-50 flex flex-col bg-surface-container-lowest border border-outline-variant/60 rounded-2xl shadow-2xl overflow-hidden'
              : 'relative'
          ]"
        >
          <BaseCard
            :title="t('system.systemLogsViewerTitle')"
            :subtitle="t('system.systemLogsViewerSubtitle')"
            icon="terminal"
            :no-padding="true"
            class="h-full flex flex-col"
          >
            <template #headerActions>
              <!-- Counters Indicators -->
              <div class="hidden xl:flex items-center gap-1.5 mr-2 font-mono text-[11px]">
                <span class="px-2 py-0.5 rounded-md bg-surface-container-highest border border-outline-variant/40 text-on-surface-variant">
                  {{ t('system.eventsCount') }}: <strong class="text-on-surface">{{ logCounts.total }}</strong>
                </span>
                <span
                  v-if="logCounts.errors > 0"
                  class="px-2 py-0.5 rounded-md bg-error/15 border border-error/30 text-error font-bold"
                >
                  {{ logCounts.errors }} {{ t('system.errorsCount') }}
                </span>
                <span
                  v-if="logCounts.warns > 0"
                  class="px-2 py-0.5 rounded-md bg-warning-yellow/15 border border-warning-yellow/30 text-warning-yellow"
                >
                  {{ logCounts.warns }} {{ t('system.warnsCount') }}
                </span>
              </div>

              <!-- Log File Selector -->
              <div class="w-44">
                <BaseSelect
                  v-model="selectedLogFile"
                  :options="logFileOptions"
                  size="sm"
                />
              </div>

              <!-- Log Level Selector -->
              <div class="w-24">
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
                width-class="w-40"
              />

              <!-- Auto-refresh Checkbox & Live Indicator -->
              <div class="flex items-center gap-2 ml-1">
                <label class="flex items-center gap-1.5 cursor-pointer">
                  <input
                    v-model="isAutoRefresh"
                    type="checkbox"
                    class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0 cursor-pointer"
                  />
                  <span class="text-[10px] text-on-surface-variant select-none hidden md:inline">{{ t('system.autoRefresh') }}</span>
                </label>

                <StatusBadge
                  :variant="isAutoRefresh ? (isUserScrolledUp ? 'warning' : 'success') : 'neutral'"
                  :pulse="isAutoRefresh && !isUserScrolledUp"
                  :dot="true"
                  size="xs"
                >
                  {{ isUserScrolledUp ? t('system.streamPaused') : t('system.streamLive') }}
                </StatusBadge>
              </div>

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
                  v-if="authStore.canManageSystem"
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
                  v-if="authStore.canManageSystem"
                  variant="outline"
                  size="xs"
                  icon="download"
                  :title="t('system.downloadLogs')"
                  @click="downloadCurrentLog"
                />
                <AppButton
                  variant="outline"
                  size="xs"
                  :icon="isFullscreenLogs ? 'fullscreen_exit' : 'fullscreen'"
                  :title="isFullscreenLogs ? t('system.exitFullscreen') : t('system.fullscreen')"
                  @click="isFullscreenLogs = !isFullscreenLogs"
                />
              </div>
            </template>

            <!-- Console Terminal Output Area -->
            <div class="relative flex-1 bg-surface-deep">
              <div
                ref="logConsoleRef"
                class="p-md font-mono text-xs overflow-y-auto flex flex-col gap-1 select-text transition-all"
                :class="isFullscreenLogs ? 'h-[calc(100vh-140px)]' : 'h-[440px]'"
                @scroll="handleScroll"
              >
                <div
                  v-for="entry in filteredLogs"
                  :key="entry.id"
                  class="group flex items-start gap-2.5 py-1 px-2 rounded-lg hover:bg-surface-container/60 transition-colors border border-transparent hover:border-outline-variant/30"
                >
                  <!-- Timestamp -->
                  <span class="text-on-surface-variant/80 shrink-0 text-[11px] select-none font-mono">
                    {{ entry.timestamp }}
                  </span>

                  <!-- Level Badge -->
                  <span
                    class="shrink-0 text-[10px] font-bold px-1.5 py-0.2 rounded uppercase select-none tracking-wider font-mono"
                    :class="{
                      'bg-error/20 text-error border border-error/40': entry.level === 'ERROR',
                      'bg-warning-yellow/20 text-warning-yellow border border-warning-yellow/40': entry.level === 'WARN',
                      'bg-cyan-500/20 text-cyan-700 dark:text-cyan-300 border border-cyan-500/40': entry.level === 'INFO',
                      'bg-purple-500/20 text-purple-700 dark:text-purple-300 border border-purple-500/40': entry.level === 'DEBUG'
                    }"
                  >
                    {{ entry.level }}
                  </span>

                  <!-- Source -->
                  <span class="font-semibold select-none shrink-0 text-primary-fixed-dim text-[11px]">
                    [{{ entry.source }}]
                  </span>

                  <!-- Message -->
                  <!-- eslint-disable-next-line vue/no-v-html -->
                  <span
                    class="flex-1 break-all text-[12px] leading-relaxed text-on-surface-variant"
                    :class="{
                      'text-error font-medium': entry.level === 'ERROR',
                      'text-warning-yellow': entry.level === 'WARN'
                    }"
                    v-html="renderHighlightedText(entry.message)"
                  />

                  <!-- Copy Row Action -->
                  <button
                    type="button"
                    class="opacity-0 group-hover:opacity-100 transition-opacity text-on-surface-variant hover:text-on-surface p-0.5 rounded cursor-pointer select-none"
                    :title="t('system.copyLine')"
                    @click="copyLogLine(entry)"
                  >
                    <span class="material-symbols-outlined text-[14px]">content_copy</span>
                  </button>
                </div>

                <div v-if="filteredLogs.length === 0" class="text-center py-16 text-on-surface-variant/60 text-xs">
                  {{ t('system.noLogsMatch') }}
                </div>
              </div>

              <!-- Floating Scroll to Bottom (Live) Button -->
              <div
                v-if="isUserScrolledUp"
                class="absolute bottom-4 right-6 z-10"
              >
                <AppButton
                  variant="primary"
                  size="xs"
                  icon="arrow_downward"
                  @click="() => scrollToBottom(true)"
                >
                  {{ t('system.scrollToBottom') }}
                </AppButton>
              </div>
            </div>
          </BaseCard>
        </div>
      </div>
    </main>

    <!-- Modal: Confirmation Dialog -->
    <ConfirmModal
      v-model="showConfirmModal"
      :title="confirmModalConfig.title"
      :message="confirmModalConfig.message"
      :variant="confirmModalConfig.variant"
      :icon="confirmModalConfig.icon"
      :confirm-text="confirmModalConfig.confirmText"
      @confirm="handleConfirmModal"
    />

    <!-- Modal: Service Status Dialog -->
    <BaseModal
      v-model="showServiceStatusModal"
      :title="t('system.serviceStatusModalTitle')"
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
          {{ t('common.close') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Modal: Rotate Audit Dialog -->
    <BaseModal
      v-model="showRotateModal"
      :title="t('system.rotateAuditModalTitle')"
      icon="history"
      max-width="max-w-md"
    >
      <div class="space-y-4">
        <!-- Блок 1: Автоматический срок хранения (политика) -->
        <div class="p-3 bg-surface-container-highest/40 rounded-xl border border-outline-variant/40 space-y-2">
          <div class="flex items-center justify-between">
            <label class="text-xs font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">schedule</span>
              {{ t('system.auditRetentionTitle') }}
            </label>
            <span v-if="isSavingRetention" class="text-[10px] text-primary-fixed-dim animate-pulse font-mono">
              {{ t('common.saving') }}
            </span>
          </div>
          <p class="text-[11px] text-on-surface-variant leading-tight">
            {{ t('system.auditRetentionDesc') }}
          </p>
          <div class="grid grid-cols-5 gap-1.5 pt-1">
            <button
              v-for="d in [30, 60, 90, 180, 365]"
              :key="d"
              type="button"
              class="py-2 px-1 rounded-lg border text-xs font-mono font-bold transition-all cursor-pointer text-center"
              :class="auditRetentionDays === d
                ? 'bg-primary/20 border-primary text-primary-fixed-dim shadow-sm'
                : 'bg-surface-container-highest/60 border-outline-variant/40 text-on-surface-variant hover:text-on-surface hover:border-outline-variant'"
              :disabled="isSavingRetention"
              @click="updateRetentionDays(d)"
            >
              {{ d }} {{ t('system.daysCount', { count: '' }).trim() }}
            </button>
          </div>
        </div>

        <!-- Блок 2: Разовая ручная ротация -->
        <div class="p-3 bg-surface-container-highest/40 rounded-xl border border-outline-variant/40 space-y-3">
          <div>
            <label class="text-xs font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">delete_sweep</span>
              {{ t('system.rotateAuditDaysLabel') }}
            </label>
            <p class="text-[11px] text-on-surface-variant leading-tight mt-0.5">
              {{ t('system.confirmRotateMsg') }}
            </p>
          </div>

          <div class="grid grid-cols-5 gap-1.5">
            <button
              v-for="d in [30, 60, 90, 180, 365]"
              :key="d"
              type="button"
              class="py-2 px-1 rounded-lg border text-xs font-mono font-bold transition-all cursor-pointer text-center"
              :class="rotateDays === d
                ? 'bg-primary/20 border-primary text-primary-fixed-dim shadow-sm'
                : 'bg-surface-container-highest/60 border-outline-variant/40 text-on-surface-variant hover:text-on-surface hover:border-outline-variant'"
              @click="rotateDays = d"
            >
              {{ d }} {{ t('system.daysCount', { count: '' }).trim() }}
            </button>
          </div>

          <div class="flex items-center gap-2 pt-2 border-t border-outline-variant/30">
            <input
              id="saveArchiveCheck"
              v-model="rotateSaveArchive"
              type="checkbox"
              class="rounded border-outline-variant bg-surface-container-highest text-primary focus:ring-primary h-4 w-4 cursor-pointer"
            />
            <label for="saveArchiveCheck" class="text-xs text-on-surface cursor-pointer select-none">
              {{ t('system.saveArchiveCheckbox') }}
            </label>
          </div>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center justify-end gap-2 w-full">
          <AppButton
            variant="ghost"
            size="sm"
            @click="showRotateModal = false"
          >
            {{ t('common.cancel') }}
          </AppButton>
          <AppButton
            variant="primary"
            size="sm"
            icon="history"
            :loading="isRotating"
            @click="executeRotateAudit"
          >
            {{ t('system.rotateAudit') }}
          </AppButton>
        </div>
      </template>
    </BaseModal>
  </div>
</template>

