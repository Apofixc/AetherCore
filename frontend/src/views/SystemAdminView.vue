<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
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
import {
  systemApi,
  type SystemInfo,
  type LogProvider,
  type DbStatsResponse,
  type BackupFileInfo
} from '@/api/system'
import { settingsApi } from '@/api/settings'
import { modulesApi } from '@/api/modules'
import SchedulerManager from '@/components/system/SchedulerManager.vue'
import { useToast } from '@/composables/useToast'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()
const toast = useToast()

// DB Stats & Backup state
const dbStats = ref<DbStatsResponse | null>(null)
const backupsList = ref<BackupFileInfo[]>([])
const showBackupsModal = ref(false)
const isCreatingBackup = ref(false)
const isRestoringBackup = ref(false)
const maintenanceSettings = ref<{
  auto_backup?: boolean
  backup_interval_hours?: number
  backup_retention_days?: number
  audit_retention_days?: number
} | null>(null)

// Retention & Rotation state
const auditRetentionDays = ref(90)
const isSavingRetention = ref(false)
const showRotateModal = ref(false)
const rotateDays = ref(90)
const rotateSaveArchive = ref(true)
const isRotating = ref(false)

interface SessionItem {
  id: string
  user_id?: string
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
  level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG' | 'TRACE'
  source: string
  message: string
}

const systemInfo = ref<SystemInfo | null>(null)
const logProviders = ref<LogProvider[]>([])
const selectedProviderId = ref('system')

// Active sessions state
const sessions = ref<SessionItem[]>([])
const isLoadingSessions = ref(false)

function formatUserAgent(ua?: string): string {
  if (!ua || ua === 'Web Client') return 'Web Client'
  let browser = 'Browser'
  let os = 'OS'

  if (ua.includes('Edg/')) browser = 'Edge'
  else if (ua.includes('Chrome/')) browser = 'Chrome'
  else if (ua.includes('Firefox/')) browser = 'Firefox'
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari'
  else if (ua.includes('Opera/') || ua.includes('OPR/')) browser = 'Opera'

  if (ua.includes('Windows')) os = 'Windows'
  else if (ua.includes('Macintosh') || ua.includes('Mac OS')) os = 'macOS'
  else if (ua.includes('Linux')) os = 'Linux'
  else if (ua.includes('Android')) os = 'Android'
  else if (ua.includes('iPhone') || ua.includes('iPad')) os = 'iOS'

  return `${browser} on ${os}`
}

function formatSessionTime(isoStr?: string): string {
  if (!isoStr) return ''
  try {
    const d = new Date(isoStr)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  } catch {
    return isoStr
  }
}

async function loadSessions() {
  isLoadingSessions.value = true
  try {
    const list = await systemApi.getSessions()
    if (list && Array.isArray(list) && list.length > 0) {
      sessions.value = list.map((item) => ({
        id: item.id,
        user_id: item.user_id,
        username: item.username,
        role: item.role || 'Operator',
        ip: item.ip_address,
        time: formatSessionTime(item.last_active_at || item.created_at),
        client: formatUserAgent(item.user_agent),
        isCurrent: item.is_current
      }))
    } else if (authStore.user) {
      sessions.value = [
        {
          id: `sess-${authStore.user.id.slice(0, 8)}`,
          user_id: authStore.user.id,
          username: authStore.user.username,
          role: authStore.user.is_superuser
            ? 'Superuser'
            : (authStore.user.roles?.[0]
                ? authStore.user.roles[0].charAt(0).toUpperCase() + authStore.user.roles[0].slice(1)
                : 'Operator'),
          ip: '127.0.0.1',
          time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
          client: 'Web Client',
          isCurrent: true
        }
      ]
    } else {
      sessions.value = []
    }
  } catch (err) {
    console.debug('Failed to load active sessions:', err)
    if (authStore.user && sessions.value.length === 0) {
      sessions.value = [
        {
          id: `sess-${authStore.user.id.slice(0, 8)}`,
          user_id: authStore.user.id,
          username: authStore.user.username,
          role: authStore.user.is_superuser ? 'Superuser' : 'Operator',
          ip: '127.0.0.1',
          time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
          client: 'Web Client',
          isCurrent: true
        }
      ]
    }
  } finally {
    isLoadingSessions.value = false
  }
}


// System logs state
const selectedLogLevel = ref('ALL')
const logSearchQuery = ref('')
const isAutoRefresh = ref(true)
const fileInputRef = ref<HTMLInputElement | null>(null)
const logConsoleRef = ref<HTMLDivElement | null>(null)
const isUserScrolledUp = ref(false)
const isFullscreenLogs = ref(false)
const showServiceStatusModal = ref(false)
let refreshTimer: number | null = null
let searchDebounce: number | null = null

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
    debug: logs.value.filter((l) => l.level === 'DEBUG' || l.level === 'TRACE').length
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

function notify(msg: string, type?: 'success' | 'error' | 'info') {
  if (type === 'error' || msg.toLowerCase().includes('error')) {
    toast.error(msg)
  } else if (type === 'success' || msg.includes('OK') || msg.includes('успешно') || msg.includes('Success')) {
    toast.success(msg)
  } else {
    toast.info(msg)
  }
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

const modulesCount = ref({ active: 0, total: 0 })

function formatUptime(seconds?: number): string {
  if (!seconds || seconds <= 0) return t('system.lessThanMin')
  const d = Math.floor(seconds / 86400)
  const h = Math.floor((seconds % 86400) / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  const parts: string[] = []
  if (d > 0) parts.push(`${d} ${t('system.daysShort')}`)
  if (h > 0) parts.push(`${h} ${t('system.hoursShort')}`)
  if (m > 0) parts.push(`${m} ${t('system.minutesShort')}`)
  if (parts.length === 0 || (d === 0 && h === 0 && s > 0)) parts.push(`${s} ${t('system.secondsShort')}`)
  return parts.join(' ')
}

async function loadModulesStats() {
  try {
    const list = await modulesApi.list()
    modulesCount.value = {
      total: list.length,
      active: list.filter((m) => m.is_active).length
    }
  } catch (err) {
    // fallback
  }
}

function formatBytes(bytes?: number): string {
  if (!bytes || bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

async function loadDbStats() {
  try {
    const res = await systemApi.getDbStats()
    if (res) {
      dbStats.value = res
    }
  } catch (err) {
    console.debug('Failed to load DB stats:', err)
  }
}

async function loadBackupsList() {
  try {
    const list = await systemApi.getBackups()
    backupsList.value = list || []
  } catch (err) {
    console.error('Failed to load backups list:', err)
  }
}

function openBackupsModal() {
  showBackupsModal.value = true
  loadBackupsList()
}

async function handleCreateBackup(tag = 'manual') {
  isCreatingBackup.value = true
  try {
    const info = await systemApi.createBackup(tag)
    notify(t('system.createBackupSuccess') + ` (${info.filename})`)
    await Promise.all([loadDbStats(), loadBackupsList()])
  } catch (err: any) {
    console.error('Failed to create backup:', err)
    notify(err?.message || 'Error creating backup')
  } finally {
    isCreatingBackup.value = false
  }
}

async function handleDownloadServerBackup(filename: string) {
  try {
    const blob = await systemApi.downloadBackup(filename)
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    document.body.appendChild(link)
    link.click()
    link.remove()
    URL.revokeObjectURL(url)
    notify(t('system.downloadBackup') + ' - OK')
  } catch (err) {
    console.error('Failed to download backup:', err)
  }
}

function requestRestoreServerBackup(b: BackupFileInfo) {
  confirmModalConfig.value = {
    title: t('system.confirmRestoreTitle'),
    message: t('system.confirmRestoreMsg', { file: b.filename }),
    variant: 'danger',
    icon: 'restore',
    confirmText: t('system.restoreFromFile'),
    action: async () => {
      isRestoringBackup.value = true
      try {
        await systemApi.restoreBackup(b.filename)
        notify(t('system.restoreSuccess'))
        await Promise.all([loadDbStats(), loadBackupsList()])
      } catch (err: any) {
        console.error('Failed to restore database:', err)
        notify(err?.message || 'Restore error')
      } finally {
        isRestoringBackup.value = false
      }
    }
  }
  showConfirmModal.value = true
}

function requestDeleteBackup(b: BackupFileInfo) {
  confirmModalConfig.value = {
    title: t('system.confirmDeleteBackupTitle'),
    message: t('system.confirmDeleteBackupMsg', { file: b.filename }),
    variant: 'danger',
    icon: 'delete',
    confirmText: t('common.delete'),
    action: async () => {
      try {
        await systemApi.deleteBackup(b.filename)
        notify(t('system.deleteBackupSuccess'))
        await Promise.all([loadDbStats(), loadBackupsList()])
      } catch (err: any) {
        console.error('Failed to delete backup:', err)
      }
    }
  }
  showConfirmModal.value = true
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
      action: async () => {
        isRestoringBackup.value = true
        try {
          await systemApi.uploadAndRestoreBackup(file)
          notify(t('system.restoreSuccess'))
          await Promise.all([loadDbStats(), loadBackupsList()])
        } catch (err: any) {
          console.error('Failed to upload and restore backup:', err)
          notify(err?.message || 'Restore error')
        } finally {
          isRestoringBackup.value = false
        }
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
    if (maint) {
      maintenanceSettings.value = maint
      if (maint.audit_retention_days) {
        auditRetentionDays.value = maint.audit_retention_days
        rotateDays.value = maint.audit_retention_days
      }
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
    action: async () => {
      try {
        await systemApi.revokeSession(s.id)
        notify(t('system.revokeSession') + ` (${s.username}) - OK`)
        await loadSessions()
      } catch (err: any) {
        console.error('Failed to revoke session:', err)
        notify(err?.message || 'Error revoking session')
      }
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
    action: async () => {
      try {
        const res = await systemApi.terminateOtherSessions()
        notify(t('system.confirmRevokeOthersTitle') + ` (${res.terminated_count}) - OK`)
        await loadSessions()
      } catch (err: any) {
        console.error('Failed to terminate other sessions:', err)
        notify(err?.message || 'Error terminating sessions')
      }
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
    action: async () => {
      try {
        await systemApi.terminateAllSessions()
      } catch (err) {
        console.debug('Failed to terminate all sessions on backend:', err)
      }
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
      limit: 200
    })
    if (res) {
      if (Array.isArray(res.entries)) {
        logs.value = res.entries.map((item, idx) => ({
          id: `${idx}-${item.timestamp}`,
          timestamp: item.timestamp ? item.timestamp.replace('T', ' ').slice(0, 19) : new Date().toISOString().replace('T', ' ').slice(0, 19),
          level: ((item.level || 'INFO').toUpperCase()) as any,
          source: item.target || 'core',
          message: item.message || item.raw || ''
        }))
      } else if (Array.isArray(res.lines)) {
        logs.value = res.lines.map((line, idx) => parseLogLine(line, idx))
      }
      if (!isUserScrolledUp.value) {
        scrollToBottom(false)
      }
    }
  } catch (e) {
    // API offline or error, maintain existing log lines
  }
}

watch([selectedProviderId, selectedLogLevel], () => {
  fetchRealLogs()
})

watch(logSearchQuery, () => {
  if (searchDebounce) clearTimeout(searchDebounce)
  searchDebounce = window.setTimeout(() => {
    fetchRealLogs()
  }, 300)
})

async function loadSystemData() {
  try {
    const [info, providers, _] = await Promise.all([
      systemApi.getInfo().catch(() => null),
      systemApi.getProviders().catch(() => []),
      loadDbStats().catch(() => null),
      loadModulesStats().catch(() => null)
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

  await loadSessions()
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
    link.download = `system_${selectedProviderId.value}_${new Date().toISOString().slice(0, 10)}.log`
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
    return logProviders.value.map((p) => {
      let label = p.name
      if (p.id === 'system') label = t('system.providerSystem')
      else if (p.id === 'server') label = t('system.providerServer')
      else if (p.id === 'scheduler') label = t('system.providerScheduler')
      else if (p.id === 'auth') label = t('system.providerAuth')
      else if (p.id === 'database') label = t('system.providerDatabase')
      else if (p.id === 'plugins') label = t('system.providerPlugins')
      return {
        value: p.id,
        label: `[${p.category || p.kind || 'system'}] ${label}`
      }
    })
  }
  return [
    { value: 'system', label: `[system] ${t('system.providerSystem')}` },
    { value: 'server', label: `[server] ${t('system.providerServer')}` },
    { value: 'scheduler', label: `[scheduler] ${t('system.providerScheduler')}` },
    { value: 'auth', label: `[auth] ${t('system.providerAuth')}` },
    { value: 'database', label: `[database] ${t('system.providerDatabase')}` },
    { value: 'plugins', label: `[plugins] ${t('system.providerPlugins')}` }
  ]
})

const logLevelOptions = [
  { value: 'ALL', label: 'ALL' },
  { value: 'ERROR', label: 'ERROR' },
  { value: 'WARN', label: 'WARN' },
  { value: 'INFO', label: 'INFO' },
  { value: 'DEBUG', label: 'DEBUG' },
  { value: 'TRACE', label: 'TRACE' }
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
  if (searchDebounce) clearTimeout(searchDebounce)
})
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">
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
            <template #headerActions v-if="authStore.canManageSystem">
              <AppButton
                variant="outline"
                size="xs"
                icon="history"
                @click="requestRotateAudit"
              >
                {{ t('system.rotateAudit') }}
              </AppButton>
              <AppButton
                variant="primary"
                size="xs"
                icon="storage"
                @click="openBackupsModal"
              >
                {{ t('system.manageBackups') }}
              </AppButton>
            </template>

            <input
              ref="fileInputRef"
              type="file"
              accept=".db,.sqlite,.sqlite3"
              class="hidden"
              @change="handleFileSelected"
            />

            <!-- Database & Backup Health Status Grid -->
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
              <div class="bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.dbSize') }}</span>
                <span class="text-xs font-mono font-bold text-on-surface mt-0.5">
                  {{ formatBytes(dbStats?.storage?.total_size_bytes || 0) }}
                  <span v-if="dbStats?.storage?.wal_size_bytes" class="text-[10px] text-on-surface-variant font-normal">
                    (WAL: {{ formatBytes(dbStats?.storage?.wal_size_bytes || 0) }})
                  </span>
                </span>
                <span class="text-[10px] text-on-surface-variant/70 font-mono">
                  {{ dbStats?.storage?.tables_count ?? 6 }} {{ t('system.tablesCount').toLowerCase() }}
                </span>
              </div>
              <div class="bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.lastBackup') }}</span>
                <span class="text-xs font-mono font-bold text-on-surface mt-0.5">
                  {{ dbStats?.latest_backup?.created_at ? dbStats.latest_backup.created_at.slice(0, 19).replace('T', ' ') : '—' }}
                </span>
                <span class="text-[10px] text-primary-fixed-dim font-mono">
                  {{ dbStats?.latest_backup?.tag ? `[${dbStats.latest_backup.tag}]` : 'Snapshot' }} • {{ dbStats?.total_backups_count ?? 0 }}
                </span>
              </div>
              <div class="col-span-2 sm:col-span-1 bg-surface-container-highest/40 p-2.5 rounded-xl border border-outline-variant/30 flex flex-col justify-between">
                <span class="text-[10px] text-on-surface-variant font-mono uppercase tracking-wider">{{ t('system.autoBackup') }}</span>
                <div class="mt-1">
                  <StatusBadge
                    :variant="maintenanceSettings?.auto_backup !== false ? 'success' : 'neutral'"
                    size="xs"
                    :dot="true"
                  >
                    {{ maintenanceSettings?.auto_backup !== false
                        ? t('system.autoBackupEnabled', { hours: maintenanceSettings?.backup_interval_hours || 24 })
                        : t('system.autoBackupDisabled')
                    }}
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
                variant="ghost"
                size="xs"
                icon="refresh"
                :loading="isLoadingSessions"
                @click="loadSessions"
              >
                {{ t('common.refresh') }}
              </AppButton>
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
                v-if="sessions.length === 0 && !isLoadingSessions"
                class="text-xs text-on-surface-variant/70 font-mono py-6 text-center"
              >
                {{ t('system.noActiveSessions') }}
              </div>
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

        <!-- Task Scheduler Management Section -->
        <SchedulerManager />

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
              <div class="w-52">
                <BaseSelect
                  v-model="selectedProviderId"
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
                      'bg-purple-500/20 text-purple-700 dark:text-purple-300 border border-purple-500/40': entry.level === 'DEBUG',
                      'bg-slate-500/20 text-slate-400 border border-slate-500/40': entry.level === 'TRACE'
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
      max-width="max-w-lg"
    >
      <div class="flex flex-col gap-2.5 text-xs font-body-mono">
        <!-- 1. AetherCore Core Daemon -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">memory</span>
              {{ t('system.serviceCoreDaemon') }} v{{ systemInfo?.version || '2.0.0' }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              {{ t('system.uptimeLabel') }}: <strong class="text-on-surface">{{ formatUptime(systemInfo?.uptime_seconds) }}</strong> • {{ systemInfo?.dev_mode ? t('system.modeDev') : (systemInfo?.safe_mode ? t('system.modeSafe') : t('system.modeProduction')) }}
            </span>
          </div>
          <StatusBadge variant="success" size="xs" :pulse="true" :dot="true">
            ONLINE
          </StatusBadge>
        </div>

        <!-- 2. WASM Plugin Runtime -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">extension</span>
              {{ t('system.serviceWasmRuntime') }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              <template v-if="systemInfo?.safe_mode">
                {{ t('system.pluginsDisabledSafeMode') }}
              </template>
              <template v-else>
                {{ t('system.activeModulesOfTotal', { active: modulesCount.active, total: modulesCount.total }) }}
              </template>
            </span>
          </div>
          <StatusBadge
            :variant="systemInfo?.safe_mode ? 'warning' : 'success'"
            size="xs"
            :dot="true"
          >
            {{ systemInfo?.safe_mode ? 'SAFE-MODE' : `READY (${modulesCount.active})` }}
          </StatusBadge>
        </div>

        <!-- 3. SQLite Embedded DB -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">database</span>
              {{ t('system.serviceDbEngine') }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              {{ t('system.dbEngineInfo', { size: formatBytes(dbStats?.storage?.total_size_bytes), tables: dbStats?.storage?.tables_count ?? 6 }) }}
            </span>
          </div>
          <StatusBadge variant="success" size="xs" :dot="true">
            SYNCED (WAL)
          </StatusBadge>
        </div>

        <!-- 4. Task Scheduler Engine -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">schedule</span>
              {{ t('system.serviceSchedulerEngine') }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              {{ t('system.schedulerDesc') }}
            </span>
          </div>
          <StatusBadge variant="success" size="xs" :dot="true">
            ACTIVE
          </StatusBadge>
        </div>

        <!-- 5. IPC Message Bus -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">hub</span>
              {{ t('system.serviceIpcBus') }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              {{ t('system.ipcDesc') }}
            </span>
          </div>
          <StatusBadge variant="success" size="xs" :dot="true">
            CONNECTED
          </StatusBadge>
        </div>

        <!-- 6. System Logging Service -->
        <div class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors">
          <div class="flex flex-col gap-0.5">
            <span class="font-bold text-on-surface flex items-center gap-1.5">
              <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">terminal</span>
              {{ t('system.serviceLoggingService') }}
            </span>
            <span class="text-[11px] text-on-surface-variant font-mono">
              {{ t('system.logBufferEventsInfo', { total: logCounts.total, errors: logCounts.errors, warns: logCounts.warns }) }}
            </span>
          </div>
          <StatusBadge variant="success" size="xs" :dot="true">
            CAPTURING
          </StatusBadge>
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

    <!-- Modal: Manage Database Backups -->
    <BaseModal
      v-model="showBackupsModal"
      :title="t('system.manageBackups')"
      icon="storage"
      max-width="max-w-2xl"
    >
      <div class="space-y-4">
        <!-- Top Toolbar inside modal -->
        <div class="flex items-center justify-between p-3 bg-surface-container-highest/40 rounded-xl border border-outline-variant/40 gap-2 flex-wrap">
          <div>
            <h4 class="text-xs font-bold text-on-surface">{{ t('system.backupsListTitle') }}</h4>
            <p class="text-[11px] text-on-surface-variant mt-0.5">
              {{ t('system.restoreWarning') }}
            </p>
          </div>
          <div class="flex items-center gap-2">
            <AppButton
              variant="outline"
              size="xs"
              icon="upload_file"
              :loading="isRestoringBackup"
              @click="triggerRestoreFile"
            >
              {{ t('system.restoreFromFile') }}
            </AppButton>
            <AppButton
              variant="primary"
              size="xs"
              icon="add"
              :loading="isCreatingBackup"
              @click="handleCreateBackup('manual')"
            >
              {{ t('system.createBackupNow') }}
            </AppButton>
          </div>
        </div>

        <!-- Backups List Table / Cards -->
        <div v-if="backupsList.length > 0" class="flex flex-col gap-2 max-h-80 overflow-y-auto pr-1">
          <div
            v-for="b in backupsList"
            :key="b.filename"
            class="flex items-center justify-between p-2.5 bg-surface-container-highest/60 rounded-xl border border-outline-variant/40 hover:border-outline-variant/70 transition-colors"
          >
            <div class="flex flex-col gap-0.5 min-w-0 pr-2">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="font-mono text-xs font-bold text-on-surface truncate">{{ b.filename }}</span>
                <StatusBadge
                  :variant="b.tag === 'auto' ? 'info' : b.tag === 'pre_restore' ? 'warning' : 'primary'"
                  size="xs"
                >
                  {{ b.tag === 'auto' ? t('system.backupTagAuto') : b.tag === 'pre_restore' ? t('system.backupTagPreRestore') : b.tag === 'upload' ? t('system.backupTagUpload') : t('system.backupTagManual') }}
                </StatusBadge>
              </div>
              <div class="flex items-center gap-2 text-[11px] text-on-surface-variant font-mono">
                <span>{{ b.created_at.slice(0, 19).replace('T', ' ') }}</span>
                <span>•</span>
                <span class="font-bold text-on-surface">{{ formatBytes(b.size_bytes) }}</span>
              </div>
            </div>

            <!-- Actions per backup -->
            <div class="flex items-center gap-1.5 shrink-0">
              <AppButton
                variant="outline"
                size="xs"
                icon="download"
                :title="t('system.downloadBackup')"
                @click="handleDownloadServerBackup(b.filename)"
              />
              <AppButton
                variant="outline"
                size="xs"
                icon="restore"
                :title="t('system.restoreFromFile')"
                :loading="isRestoringBackup"
                @click="requestRestoreServerBackup(b)"
              />
              <AppButton
                variant="danger"
                size="xs"
                icon="delete"
                :title="t('common.delete')"
                @click="requestDeleteBackup(b)"
              />
            </div>
          </div>
        </div>

        <div v-else class="text-center py-10 text-on-surface-variant text-xs">
          <span class="material-symbols-outlined text-3xl opacity-40 mb-1">cloud_off</span>
          <p>{{ t('system.noBackupsFound') }}</p>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center justify-end w-full">
          <AppButton
            variant="ghost"
            size="sm"
            @click="showBackupsModal = false"
          >
            {{ t('common.close') }}
          </AppButton>
        </div>
      </template>
    </BaseModal>
  </div>
</template>

