<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  BaseCard,
  AppButton,
  StatusBadge,
  BaseModal,
  ConfirmModal,
  BaseInput,
  BaseSelect,
  BaseSwitch,
  NumberInput
} from '@/components/common'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'
import { useToast } from '@/composables/useToast'
import {
  schedulerApi,
  type ScheduledTask,
  type TaskExecutionRecord,
  type CreateTaskDto,
  type UpdateTaskDto,
  type ConcurrencyPolicy
} from '@/api/scheduler'

const { t } = useI18n()
const authStore = useAuthStore()
const toast = useToast()

const tasks = ref<ScheduledTask[]>([])
const isLoading = ref(false)
const executingTasks = ref<Record<string, boolean>>({})

// Select options
const scheduleTypeOptions = computed(() => [
  { value: 'cron', label: t('scheduler.scheduleTypeCron'), icon: 'schedule' },
  { value: 'interval_sec', label: t('scheduler.scheduleTypeInterval'), icon: 'timer' }
])

const actionTypeOptions = computed(() => [
  { value: 'system_history_cleanup', label: t('scheduler.actionSystemCleanup'), icon: 'cleaning_services' },
  { value: 'system_audit_rotation', label: t('scheduler.actionSystemAudit'), icon: 'history_toggle_drop_down' },
  { value: 'system_db_backup', label: t('scheduler.actionSystemBackup'), icon: 'database' }
])

const concurrencyOptions = computed(() => [
  { value: 'skip', label: t('scheduler.policySkip') },
  { value: 'allow', label: t('scheduler.policyAllow') },
  { value: 'queue', label: t('scheduler.policyQueue') }
])

// History Modal State
const showHistoryModal = ref(false)
const historyTask = ref<ScheduledTask | null>(null)
const historyRecords = ref<TaskExecutionRecord[]>([])
const isLoadingHistory = ref(false)
const pruneDays = ref(30)
const isPruning = ref(false)

// Edit / Create Modal State
const showTaskModal = ref(false)
const isEditing = ref(false)
const currentTaskId = ref<string | null>(null)
const isSaving = ref(false)

const taskForm = ref<{
  name: string
  description: string
  scheduleType: 'cron' | 'interval_sec'
  scheduleValue: string
  actionType: 'system_audit_rotation' | 'system_history_cleanup' | 'system_db_backup'
  concurrencyPolicy: ConcurrencyPolicy
  timeoutSecs: number
  isEnabled: boolean
}>({
  name: '',
  description: '',
  scheduleType: 'cron',
  scheduleValue: '0 0 * * *',
  actionType: 'system_history_cleanup',
  concurrencyPolicy: 'skip',
  timeoutSecs: 300,
  isEnabled: true
})

// Delete Confirm Modal State
const showDeleteModal = ref(false)
const taskToDelete = ref<ScheduledTask | null>(null)

function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (type === 'error') {
    toast.error(message)
  } else {
    toast.success(message)
  }
}

async function loadTasks() {
  try {
    isLoading.value = true
    tasks.value = await schedulerApi.getTasks()
  } catch (err: any) {
    showToast(err.message || 'Error loading tasks', 'error')
  } finally {
    isLoading.value = false
  }
}

async function handleRunNow(task: ScheduledTask) {
  if (executingTasks.value[task.id]) return
  try {
    executingTasks.value[task.id] = true
    const result = await schedulerApi.runTaskNow(task.id)
    if (result.status === 'success') {
      showToast(t('scheduler.taskRunSuccess', { ms: result.duration_ms }))
    } else {
      showToast(t('scheduler.taskRunFailed', { error: result.error_message || result.status }), 'error')
    }
    await loadTasks()
  } catch (err: any) {
    showToast(err.message || 'Execution error', 'error')
  } finally {
    executingTasks.value[task.id] = false
  }
}

async function handleToggle(task: ScheduledTask) {
  try {
    const updated = await schedulerApi.toggleTask(task.id, !task.is_enabled)
    const idx = tasks.value.findIndex((t) => t.id === task.id)
    if (idx !== -1) {
      tasks.value[idx] = updated
    }
    showToast(updated.is_enabled ? t('common.active') : t('common.disabled'))
  } catch (err: any) {
    showToast(err.message || 'Toggle error', 'error')
  }
}

async function openHistoryModal(task?: ScheduledTask) {
  historyTask.value = task || null
  showHistoryModal.value = true
  await fetchHistory()
}

async function fetchHistory() {
  try {
    isLoadingHistory.value = true
    if (historyTask.value) {
      historyRecords.value = await schedulerApi.getTaskHistory(historyTask.value.id, { limit: 50 })
    } else {
      historyRecords.value = await schedulerApi.getAllHistory({ limit: 50 })
    }
  } catch (err: any) {
    showToast(err.message || 'Error fetching history', 'error')
  } finally {
    isLoadingHistory.value = false
  }
}

async function handlePruneHistory() {
  try {
    isPruning.value = true
    const res = await schedulerApi.pruneHistory(pruneDays.value)
    showToast(t('scheduler.pruneSuccess', { count: res.deleted_count }))
    await fetchHistory()
  } catch (err: any) {
    showToast(err.message || 'Prune error', 'error')
  } finally {
    isPruning.value = false
  }
}

function openCreateModal() {
  isEditing.value = false
  currentTaskId.value = null
  taskForm.value = {
    name: '',
    description: '',
    scheduleType: 'cron',
    scheduleValue: '0 */6 * * *',
    actionType: 'system_history_cleanup',
    concurrencyPolicy: 'skip',
    timeoutSecs: 300,
    isEnabled: true
  }
  showTaskModal.value = true
}

function openEditModal(task: ScheduledTask) {
  if (task.is_system) return
  isEditing.value = true
  currentTaskId.value = task.id
  taskForm.value = {
    name: task.name,
    description: task.description || '',
    scheduleType: task.schedule.type === 'interval_sec' ? 'interval_sec' : 'cron',
    scheduleValue: task.schedule.type === 'cron' ? task.schedule.value : String((task.schedule as any).value || 300),
    actionType: task.action.type === 'system_audit_rotation' ? 'system_audit_rotation' : 'system_history_cleanup',
    concurrencyPolicy: task.concurrency_policy,
    timeoutSecs: task.timeout_secs,
    isEnabled: task.is_enabled
  }
  showTaskModal.value = true
}

async function handleSaveTask() {
  if (!taskForm.value.name.trim()) return

  try {
    isSaving.value = true
    const schedule =
      taskForm.value.scheduleType === 'cron'
        ? { type: 'cron' as const, value: taskForm.value.scheduleValue.trim() }
        : { type: 'interval_sec' as const, value: parseInt(taskForm.value.scheduleValue, 10) || 300 }

    const action = { type: taskForm.value.actionType }

    if (isEditing.value && currentTaskId.value) {
      const updateDto: UpdateTaskDto = {
        name: taskForm.value.name.trim(),
        description: taskForm.value.description.trim() || undefined,
        schedule,
        action,
        concurrency_policy: taskForm.value.concurrencyPolicy,
        timeout_secs: taskForm.value.timeoutSecs,
        is_enabled: taskForm.value.isEnabled
      }
      await schedulerApi.updateTask(currentTaskId.value, updateDto)
      showToast(t('common.changesApplied'))
    } else {
      const createDto: CreateTaskDto = {
        name: taskForm.value.name.trim(),
        description: taskForm.value.description.trim() || undefined,
        schedule,
        action,
        concurrency_policy: taskForm.value.concurrencyPolicy,
        timeout_secs: taskForm.value.timeoutSecs,
        is_enabled: taskForm.value.isEnabled
      }
      await schedulerApi.createTask(createDto)
      showToast(t('common.changesApplied'))
    }
    showTaskModal.value = false
    await loadTasks()
  } catch (err: any) {
    showToast(err.message || 'Save error', 'error')
  } finally {
    isSaving.value = false
  }
}

function confirmDeleteTask(task: ScheduledTask) {
  if (task.is_system) return
  taskToDelete.value = task
  showDeleteModal.value = true
}

async function handleDeleteTask() {
  if (!taskToDelete.value) return
  try {
    await schedulerApi.deleteTask(taskToDelete.value.id)
    showToast(t('common.changesApplied'))
    showDeleteModal.value = false
    taskToDelete.value = null
    await loadTasks()
  } catch (err: any) {
    showToast(err.message || 'Delete error', 'error')
  }
}

function formatSchedule(sched: any): string {
  if (!sched) return '-'
  if (sched.type === 'cron') {
    return sched.value
  }
  if (sched.type === 'interval_sec') {
    return `${sched.value}s`
  }
  if (sched.type === 'one_off') {
    return new Date(sched.value).toLocaleString()
  }
  return String(sched.value || '')
}

function formatDateTime(val?: string): string {
  if (!val) return '-'
  try {
    const d = new Date(val)
    return d.toLocaleString()
  } catch {
    return val
  }
}

function getActionLabel(action: any): string {
  if (!action) return '-'
  switch (action.type) {
    case 'system_audit_rotation':
      return t('scheduler.actionSystemAudit')
    case 'system_history_cleanup':
      return t('scheduler.actionSystemCleanup')
    case 'system_db_backup':
      return t('scheduler.actionSystemBackup')
    case 'plugin_timer':
      return `${t('scheduler.actionPluginTimer')}: ${action.params?.module_id || ''}`
    case 'event_bus_publish':
      return `${t('scheduler.actionEventPublish')}: ${action.params?.topic || ''}`
    default:
      return action.type
  }
}

let poller: number | null = null

onMounted(() => {
  loadTasks()
  poller = window.setInterval(() => {
    loadTasks()
  }, 10000)
})

onUnmounted(() => {
  if (poller) clearInterval(poller)
})
</script>

<template>
  <BaseCard
    :title="t('scheduler.title')"
    :subtitle="t('scheduler.subtitle')"
    icon="schedule"
    :no-padding="true"
    class="flex flex-col"
  >
    <!-- Header Actions -->
    <template #headerActions>
      <div class="flex items-center gap-2">
        <AppButton
          variant="outline"
          size="xs"
          icon="history"
          :title="t('scheduler.allHistoryTitle')"
          @click="() => openHistoryModal()"
        >
          <span class="hidden sm:inline">{{ t('scheduler.executionHistory') }}</span>
        </AppButton>
        <AppButton
          v-if="authStore.canManageSystem"
          variant="primary"
          size="xs"
          icon="add"
          @click="openCreateModal"
        >
          {{ t('scheduler.addTask') }}
        </AppButton>
        <AppButton
          variant="outline"
          size="xs"
          icon="refresh"
          :loading="isLoading"
          @click="loadTasks"
        />
      </div>
    </template>

    <!-- Tasks Table -->
    <div class="overflow-x-auto">
      <table class="w-full text-left border-collapse">
        <thead>
          <tr class="border-b border-outline-variant/30 bg-surface-container-highest/30 text-[11px] font-mono uppercase tracking-wider text-on-surface-variant">
            <th class="py-3 px-4 font-semibold">{{ t('scheduler.taskName') }}</th>
            <th class="py-3 px-3 font-semibold">{{ t('scheduler.schedule') }}</th>
            <th class="py-3 px-3 font-semibold">{{ t('scheduler.action') }}</th>
            <th class="py-3 px-3 font-semibold">{{ t('scheduler.nextRun') }}</th>
            <th class="py-3 px-3 font-semibold">{{ t('scheduler.lastStatus') }}</th>
            <th class="py-3 px-4 font-semibold text-right">{{ t('common.actions') }}</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-outline-variant/20 text-xs">
          <tr
            v-for="task in tasks"
            :key="task.id"
            class="hover:bg-surface-container-high/40 transition-colors"
          >
            <!-- Name & Badges -->
            <td class="py-3 px-4">
              <div class="flex flex-col gap-1">
                <div class="flex items-center gap-2">
                  <span class="font-bold text-on-surface font-mono">{{ task.name }}</span>
                  <span
                    class="text-[9px] px-1.5 py-0.2 rounded font-mono font-bold uppercase tracking-wider"
                    :class="task.is_system ? 'bg-primary-fixed-dim/20 text-primary-fixed-dim border border-primary-fixed-dim/30' : 'bg-surface-container-highest text-on-surface-variant'"
                  >
                    {{ task.is_system ? t('scheduler.systemTaskBadge') : t('scheduler.userTaskBadge') }}
                  </span>
                </div>
                <span v-if="task.description" class="text-[11px] text-on-surface-variant/80">
                  {{ task.description }}
                </span>
              </div>
            </td>

            <!-- Schedule -->
            <td class="py-3 px-3 font-mono text-[11px]">
              <span class="px-2 py-0.5 rounded bg-surface-container-highest border border-outline-variant/30 text-on-surface">
                {{ formatSchedule(task.schedule) }}
              </span>
            </td>

            <!-- Action -->
            <td class="py-3 px-3">
              <span class="text-xs text-on-surface-variant font-medium">
                {{ getActionLabel(task.action) }}
              </span>
            </td>

            <!-- Next Run -->
            <td class="py-3 px-3 font-mono text-[11px]">
              <span
                v-if="task.is_enabled && task.next_run_at"
                class="text-on-surface cursor-help"
                :title="'UTC: ' + task.next_run_at"
              >
                {{ formatDateTime(task.next_run_at) }}
              </span>
              <span v-else class="text-on-surface-variant/50 italic">
                {{ task.is_enabled ? '-' : t('common.disabled') }}
              </span>
            </td>

            <!-- Status Badge -->
            <td class="py-3 px-3">
              <div class="flex items-center gap-1.5">
                <StatusBadge
                  v-if="!task.is_enabled"
                  variant="neutral"
                  size="xs"
                >
                  {{ t('common.disabled') }}
                </StatusBadge>
                <StatusBadge
                  v-else-if="executingTasks[task.id] || (task.last_status as any) === 'running'"
                  variant="primary"
                  :pulse="true"
                  size="xs"
                >
                  {{ t('scheduler.running') }}
                </StatusBadge>
                <StatusBadge
                  v-else-if="task.last_status === 'success'"
                  variant="success"
                  :dot="true"
                  size="xs"
                >
                  OK
                </StatusBadge>
                <StatusBadge
                  v-else-if="task.last_status === 'failed' || task.last_status === 'timeout'"
                  variant="danger"
                  :dot="true"
                  size="xs"
                >
                  {{ task.last_status }}
                </StatusBadge>
                <StatusBadge
                  v-else
                  variant="neutral"
                  size="xs"
                >
                  idle
                </StatusBadge>
              </div>
            </td>

            <!-- Action Buttons -->
            <td class="py-3 px-4 text-right">
              <div class="flex items-center justify-end gap-1">
                <!-- Run Now -->
                <AppButton
                  v-if="authStore.canManageSystem"
                  variant="outline"
                  size="xs"
                  icon="play_arrow"
                  :loading="executingTasks[task.id]"
                  :title="t('scheduler.runNow')"
                  @click="() => handleRunNow(task)"
                />

                <!-- Toggle active -->
                <AppButton
                  v-if="authStore.canManageSystem"
                  variant="outline"
                  size="xs"
                  :icon="task.is_enabled ? 'pause' : 'play_circle'"
                  :title="task.is_enabled ? t('common.disabled') : t('common.active')"
                  @click="() => handleToggle(task)"
                />

                <!-- History -->
                <AppButton
                  variant="outline"
                  size="xs"
                  icon="history"
                  :title="t('scheduler.historyTitle', { name: task.name })"
                  @click="() => openHistoryModal(task)"
                />

                <!-- Edit (Non-system) -->
                <AppButton
                  v-if="!task.is_system && authStore.canManageSystem"
                  variant="outline"
                  size="xs"
                  icon="edit"
                  :title="t('common.edit')"
                  @click="() => openEditModal(task)"
                />

                <!-- Delete (Non-system) -->
                <AppButton
                  v-if="!task.is_system && authStore.canManageSystem"
                  variant="danger"
                  size="xs"
                  icon="delete"
                  :title="t('common.delete')"
                  @click="() => confirmDeleteTask(task)"
                />
              </div>
            </td>
          </tr>

          <tr v-if="tasks.length === 0 && !isLoading">
            <td colspan="6" class="py-8 text-center text-on-surface-variant/60 font-mono text-xs">
              {{ t('scheduler.noTasks') }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- History Modal -->
    <BaseModal
      v-model="showHistoryModal"
      :title="historyTask ? t('scheduler.historyTitle', { name: historyTask.name }) : t('scheduler.allHistoryTitle')"
      max-width="max-w-4xl"
    >
      <div class="flex flex-col gap-4">
        <!-- Prune history controls -->
        <div
          v-if="authStore.canManageSystem"
          class="flex items-center justify-between p-3 rounded-xl bg-surface-container-highest/40 border border-outline-variant/30 flex-wrap gap-3"
        >
          <div class="flex items-center gap-2.5">
            <span class="text-xs text-on-surface-variant font-medium">{{ t('scheduler.pruneHistory') }}:</span>
            <NumberInput
              v-model="pruneDays"
              :min="1"
              :max="365"
              width-class="w-20"
            />
            <span class="text-xs text-on-surface-variant">{{ t('scheduler.daysUnit') }}</span>
          </div>
          <AppButton
            variant="outline"
            size="xs"
            icon="delete_sweep"
            :loading="isPruning"
            @click="handlePruneHistory"
          >
            {{ t('common.clear') }}
          </AppButton>
        </div>

        <!-- History records table -->
        <div class="max-h-96 overflow-y-auto border border-outline-variant/30 rounded-xl">
          <table class="w-full text-left border-collapse">
            <thead>
              <tr class="border-b border-outline-variant/30 bg-surface-container-highest/50 text-[10px] font-mono uppercase tracking-wider text-on-surface-variant">
                <th class="py-2.5 px-3">{{ t('scheduler.startedAt') }}</th>
                <th v-if="!historyTask" class="py-2.5 px-3">{{ t('scheduler.taskName') }}</th>
                <th class="py-2.5 px-3">{{ t('scheduler.duration') }}</th>
                <th class="py-2.5 px-3">{{ t('scheduler.status') }}</th>
                <th class="py-2.5 px-3">{{ t('scheduler.triggeredBy') }}</th>
                <th class="py-2.5 px-3">{{ t('scheduler.details') }}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-outline-variant/20 font-mono text-[11px]">
              <tr
                v-for="rec in historyRecords"
                :key="rec.id"
                class="hover:bg-surface-container-highest/30"
              >
                <td class="py-2 px-3 text-on-surface">{{ formatDateTime(rec.started_at) }}</td>
                <td v-if="!historyTask" class="py-2 px-3 text-on-surface font-bold">{{ rec.task_name }}</td>
                <td class="py-2 px-3 text-on-surface-variant">{{ rec.duration_ms }} ms</td>
                <td class="py-2 px-3">
                  <span
                    class="px-1.5 py-0.2 rounded text-[10px] font-bold uppercase"
                    :class="{
                      'bg-success/20 text-success': rec.status === 'success',
                      'bg-error/20 text-error': rec.status === 'failed' || rec.status === 'timeout',
                      'bg-warning-yellow/20 text-warning-yellow': rec.status === 'skipped',
                      'bg-neutral/20 text-neutral': rec.status === 'aborted'
                    }"
                  >
                    {{ rec.status }}
                  </span>
                </td>
                <td class="py-2 px-3 text-on-surface-variant">{{ rec.triggered_by }}</td>
                <td class="py-2 px-3 text-on-surface-variant/80 max-w-xs truncate" :title="rec.error_message || ''">
                  {{ rec.error_message || '-' }}
                </td>
              </tr>
              <tr v-if="historyRecords.length === 0 && !isLoadingHistory">
                <td :colspan="historyTask ? 5 : 6" class="py-8 text-center text-on-surface-variant/60 text-xs">
                  {{ t('scheduler.noHistory') }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <template #footer>
        <AppButton variant="outline" size="sm" @click="showHistoryModal = false">
          {{ t('common.close') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Create / Edit Task Modal -->
    <BaseModal
      v-model="showTaskModal"
      :title="isEditing ? t('scheduler.editTask') : t('scheduler.addTask')"
      max-width="max-w-xl"
    >
      <form class="flex flex-col gap-4" @submit.prevent="handleSaveTask">
        <!-- Name -->
        <BaseInput
          v-model="taskForm.name"
          :label="t('scheduler.taskName')"
          placeholder="Например: Ночная ротация"
          icon="title"
          :required="true"
        />

        <!-- Description -->
        <BaseInput
          v-model="taskForm.description"
          :label="t('scheduler.taskDescription')"
          placeholder="Краткое назначение задачи"
          icon="description"
        />

        <!-- Schedule Type & Value -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <BaseSelect
            v-model="taskForm.scheduleType"
            :options="scheduleTypeOptions"
            :label="t('scheduler.schedule') + ' (Тип)'"
            icon="schedule"
          />

          <BaseInput
            v-model="taskForm.scheduleValue"
            :label="t('scheduler.schedule') + ' (Значение)'"
            :placeholder="taskForm.scheduleType === 'cron' ? t('scheduler.cronPlaceholder') : t('scheduler.intervalPlaceholder')"
            icon="tune"
            :required="true"
          />
        </div>

        <!-- Action Type -->
        <BaseSelect
          v-model="taskForm.actionType"
          :options="actionTypeOptions"
          :label="t('scheduler.action')"
          icon="play_circle"
        />

        <!-- Concurrency & Timeout -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <BaseSelect
            v-model="taskForm.concurrencyPolicy"
            :options="concurrencyOptions"
            :label="t('scheduler.concurrencyPolicy')"
            icon="alt_route"
          />

          <BaseInput
            v-model="taskForm.timeoutSecs"
            type="number"
            :label="t('scheduler.timeoutSecs')"
            icon="timer"
            :required="true"
          />
        </div>

        <!-- Is Enabled Switch -->
        <BaseSwitch
          v-model="taskForm.isEnabled"
          :label="t('common.active')"
          description="Включить регулярное автоматическое исполнение задачи по расписанию"
          icon="toggle_on"
          :card="true"
        />
      </form>

      <template #footer>
        <div class="flex items-center justify-end gap-2">
          <AppButton variant="outline" size="sm" @click="showTaskModal = false">
            {{ t('common.cancel') }}
          </AppButton>
          <AppButton
            variant="primary"
            size="sm"
            :loading="isSaving"
            @click="handleSaveTask"
          >
            {{ t('common.save') }}
          </AppButton>
        </div>
      </template>
    </BaseModal>

    <!-- Confirm Delete Modal -->
    <ConfirmModal
      v-model="showDeleteModal"
      :title="t('scheduler.deleteTask')"
      :message="t('scheduler.deleteTaskConfirm', { name: taskToDelete?.name || '' })"
      variant="danger"
      @confirm="handleDeleteTask"
    />
  </BaseCard>
</template>
