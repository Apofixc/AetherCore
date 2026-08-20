<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  SearchInput,
  BaseSelect,
  AppButton,
  StatusBadge,
  BaseModal,
  ConfirmModal,
  BaseInput
} from '@/components/common'
import { useI18n } from '@/i18n'
import { usersApi } from '@/api/users'
import type { User } from '@/api/auth'

const { t } = useI18n()

interface OperatorItem {
  id: string
  uid: string
  username: string
  full_name: string
  email: string
  role: 'superuser' | 'admin' | 'operator' | 'viewer'
  is_online: boolean
  is_active: boolean
  initials: string
}

const loading = ref(false)
const searchQuery = ref('')
const statusFilter = ref('all')
const selectedUserIds = ref<string[]>([])
const showAddModal = ref(false)
const showLockModal = ref(false)
const selectedUserForAction = ref<OperatorItem | null>(null)
const isSubmitting = ref(false)

// Form for new user
const newUserForm = ref({
  full_name: '',
  username: '',
  email: '',
  password: '',
  role: 'operator' as 'superuser' | 'admin' | 'operator' | 'viewer'
})

// Operators list
const operators = ref<OperatorItem[]>([
  {
    id: 'ROOT-001',
    uid: 'UID: ROOT-001',
    username: 'root',
    full_name: 'Главный администратор (Root)',
    email: 'root@nms.local',
    role: 'superuser',
    is_online: true,
    is_active: true,
    initials: 'GA'
  },
  {
    id: 'UID-D6A22E',
    uid: 'UID: UID-D6A22E',
    username: 'lockout_test_user',
    full_name: 'Lockout Test',
    email: 'lockout@nms.local',
    role: 'superuser',
    is_online: false,
    is_active: true,
    initials: 'LT'
  },
  {
    id: 'UID-A1B2C3',
    uid: 'UID: UID-A1B2C3',
    username: 's.jenkins',
    full_name: 'Sarah Jenkins',
    email: 's.jenkins@nms.local',
    role: 'admin',
    is_online: true,
    is_active: true,
    initials: 'SJ'
  },
  {
    id: 'UID-F4G5H6',
    uid: 'UID: UID-F4G5H6',
    username: 'm.vance',
    full_name: 'Marcus Vance',
    email: 'm.vance@nms.local',
    role: 'operator',
    is_online: false,
    is_active: true,
    initials: 'MV'
  },
  {
    id: 'UID-J7K8L9',
    uid: 'UID: UID-J7K8L9',
    username: 'e.rodriguez',
    full_name: 'Elena Rodriguez',
    email: 'e.rodriguez@nms.local',
    role: 'viewer',
    is_online: true,
    is_active: true,
    initials: 'ER'
  },
  {
    id: 'UID-M0N1P2',
    uid: 'UID: UID-M0N1P2',
    username: 'd.kim',
    full_name: 'David Kim',
    email: 'd.kim@nms.local',
    role: 'operator',
    is_online: true,
    is_active: true,
    initials: 'DK'
  }
])

onMounted(async () => {
  loading.value = true
  try {
    const list = await usersApi.list()
    if (list && list.length > 0) {
      operators.value = list.map((u: User) => {
        const role = (u.roles && u.roles.includes('superuser')) ? 'superuser'
          : (u.roles && u.roles.includes('admin')) ? 'admin'
          : (u.roles && u.roles.includes('operator')) ? 'operator' : 'viewer'
        const initials = u.full_name
          ? u.full_name.split(' ').map((n: string) => n[0]).join('').slice(0, 2).toUpperCase()
          : u.username.slice(0, 2).toUpperCase()
        return {
          id: u.id,
          uid: `UID: ${u.id}`,
          username: u.username,
          full_name: u.full_name || u.username,
          email: u.email || `${u.username}@nms.local`,
          role,
          is_online: Boolean(u.is_active),
          is_active: Boolean(u.is_active),
          initials
        }
      })
    }
  } catch (e) {
    // Keep local default list if API is unreachable
  } finally {
    loading.value = false
  }
})

// Filtered operators
const filteredOperators = computed(() => {
  return operators.value.filter((op) => {
    // Search query
    const q = searchQuery.value.toLowerCase().trim()
    const matchesSearch = !q ||
      op.full_name.toLowerCase().includes(q) ||
      op.username.toLowerCase().includes(q) ||
      op.email.toLowerCase().includes(q) ||
      op.uid.toLowerCase().includes(q)

    if (!matchesSearch) return false

    // Status / Role filter
    if (statusFilter.value === 'all') return true
    if (statusFilter.value === 'online') return op.is_online
    if (statusFilter.value === 'offline') return !op.is_online
    if (statusFilter.value === 'superuser') return op.role === 'superuser'
    if (statusFilter.value === 'admin') return op.role === 'admin'
    if (statusFilter.value === 'operator') return op.role === 'operator'
    if (statusFilter.value === 'viewer') return op.role === 'viewer'

    return true
  })
})

const activeCount = computed(() => {
  return operators.value.filter((op) => op.is_online).length
})

const isAllSelected = computed(() => {
  return filteredOperators.value.length > 0 &&
    filteredOperators.value.every((op) => selectedUserIds.value.includes(op.id))
})

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedUserIds.value = []
  } else {
    selectedUserIds.value = filteredOperators.value.map((op) => op.id)
  }
}

function toggleSelectUser(id: string) {
  const index = selectedUserIds.value.indexOf(id)
  if (index >= 0) {
    selectedUserIds.value.splice(index, 1)
  } else {
    selectedUserIds.value.push(id)
  }
}

function handleExportCsv() {
  const header = ['ID', 'Full Name', 'Username', 'Email', 'Role', 'Status']
  const rows = filteredOperators.value.map((op) => [
    op.id,
    `"${op.full_name}"`,
    op.username,
    op.email,
    op.role,
    op.is_online ? 'Online' : 'Offline'
  ])
  const csvContent = 'data:text/csv;charset=utf-8,' + [header.join(','), ...rows.map((r) => r.join(','))].join('\n')
  const link = document.createElement('a')
  link.setAttribute('href', encodeURI(csvContent))
  link.setAttribute('download', `operators_${new Date().toISOString().slice(0, 10)}.csv`)
  document.body.appendChild(link)
  link.click()
  link.remove()
}

function handleExportJson() {
  const dataStr = 'data:text/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(filteredOperators.value, null, 2))
  const link = document.createElement('a')
  link.setAttribute('href', dataStr)
  link.setAttribute('download', `operators_${new Date().toISOString().slice(0, 10)}.json`)
  document.body.appendChild(link)
  link.click()
  link.remove()
}

function handleCreateUser() {
  if (!newUserForm.value.username.trim()) return
  isSubmitting.value = true
  setTimeout(() => {
    const id = `UID-${Math.random().toString(36).substring(2, 8).toUpperCase()}`
    const initials = newUserForm.value.full_name
      ? newUserForm.value.full_name.split(' ').map((n) => n[0]).join('').slice(0, 2).toUpperCase()
      : newUserForm.value.username.slice(0, 2).toUpperCase()

    operators.value.unshift({
      id,
      uid: `UID: ${id}`,
      username: newUserForm.value.username.trim(),
      full_name: newUserForm.value.full_name.trim() || newUserForm.value.username.trim(),
      email: newUserForm.value.email.trim() || `${newUserForm.value.username.trim()}@nms.local`,
      role: newUserForm.value.role,
      is_online: true,
      is_active: true,
      initials
    })

    newUserForm.value = {
      full_name: '',
      username: '',
      email: '',
      password: '',
      role: 'operator'
    }
    showAddModal.value = false
    isSubmitting.value = false
  }, 400)
}

function handleDeleteUser(id: string) {
  operators.value = operators.value.filter((op) => op.id !== id)
  selectedUserIds.value = selectedUserIds.value.filter((uid) => uid !== id)
}

function handleToggleLock(op: OperatorItem) {
  selectedUserForAction.value = op
  showLockModal.value = true
}

function confirmToggleLock() {
  if (selectedUserForAction.value) {
    selectedUserForAction.value.is_active = !selectedUserForAction.value.is_active
    selectedUserForAction.value.is_online = selectedUserForAction.value.is_active
  }
  showLockModal.value = false
}

const statusOptions = computed(() => [
  { value: 'all', label: t('users.allStatuses') },
  { value: 'online', label: t('users.onlineOnly') },
  { value: 'offline', label: t('users.offlineOnly') },
  { value: 'superuser', label: t('users.superusers') },
  { value: 'admin', label: t('users.administrators') },
  { value: 'operator', label: t('users.operators') },
  { value: 'viewer', label: t('users.viewers') }
])

const roleOptions = computed(() => [
  { value: 'superuser', label: t('accessIdentity.superuser') },
  { value: 'admin', label: t('accessIdentity.administrator') },
  { value: 'operator', label: t('accessIdentity.operator') },
  { value: 'viewer', label: t('accessIdentity.viewer') }
])
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
          :title="t('users.title')"
          :subtitle="t('users.subtitle')"
          icon="manage_accounts"
        />

        <!-- Action Bar: Search, Filters, Export, Add User -->
        <div class="flex items-center justify-between flex-wrap gap-md bg-surface-container-low p-md rounded-xl border border-outline-variant/60 shadow-card-dark">
          <div class="flex items-center gap-md flex-wrap">
            <!-- Search Input -->
            <SearchInput
              v-model="searchQuery"
              :placeholder="t('users.filterOperators')"
              width-class="w-64"
            />

            <!-- Role / Status Select -->
            <div class="w-48">
              <BaseSelect
                v-model="statusFilter"
                :options="statusOptions"
                size="sm"
              />
            </div>

            <!-- Active Operators Counter -->
            <span class="text-xs text-on-surface-variant font-mono hidden xl:inline-block">
              {{ t('users.showingActiveOperators', { count: activeCount }) }}
            </span>
          </div>

          <!-- Actions: Export CSV, Export JSON, Add User -->
          <div class="flex items-center gap-2 flex-wrap">
            <AppButton
              variant="outline"
              size="sm"
              @click="handleExportCsv"
              :title="t('users.exportCsv')"
            >
              CSV
            </AppButton>
            <AppButton
              variant="outline"
              size="sm"
              @click="handleExportJson"
              :title="t('users.exportJson')"
            >
              JSON
            </AppButton>
            <AppButton
              variant="primary"
              size="sm"
              icon="person_add"
              @click="showAddModal = true"
            >
              {{ t('users.addNewUser') }}
            </AppButton>
          </div>
        </div>

        <!-- Users Table Card -->
        <div class="bg-surface-container-low border border-outline-variant/60 rounded-xl overflow-hidden shadow-card-dark">
          <div class="overflow-x-auto">
            <table class="w-full text-left border-collapse">
              <thead class="bg-surface-container-highest/60 text-[10px] text-on-surface-variant uppercase font-bold tracking-wider border-b border-outline-variant/60">
                <tr>
                  <th class="py-3 px-md w-12 text-center">
                    <input
                      type="checkbox"
                      :checked="isAllSelected"
                      @change="toggleSelectAll"
                      class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0 cursor-pointer"
                    />
                  </th>
                  <th class="py-3 px-md">
                    {{ t('users.userCol') }}
                  </th>
                  <th class="py-3 px-md">
                    {{ t('users.usernameIdCol') }}
                  </th>
                  <th class="py-3 px-md">
                    {{ t('users.roleCol') }}
                  </th>
                  <th class="py-3 px-md">
                    {{ t('users.statusCol') }}
                  </th>
                  <th class="py-3 px-md text-right">
                    {{ t('users.actionsCol') }}
                  </th>
                </tr>
              </thead>
              <tbody class="divide-y divide-outline-variant/30 text-xs">
                <tr
                  v-for="op in filteredOperators"
                  :key="op.id"
                  class="hover:bg-surface-variant/20 transition-colors"
                  :class="{ 'bg-surface-variant/30': selectedUserIds.includes(op.id) }"
                >
                  <!-- Checkbox -->
                  <td class="py-3 px-md text-center">
                    <input
                      type="checkbox"
                      :checked="selectedUserIds.includes(op.id)"
                      @change="toggleSelectUser(op.id)"
                      class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0 cursor-pointer"
                    />
                  </td>

                  <!-- User Avatar & Full Name -->
                  <td class="py-3 px-md">
                    <div class="flex items-center gap-md">
                      <div
                        class="w-10 h-10 rounded-lg flex items-center justify-center font-bold font-mono shrink-0 border"
                        :class="op.role === 'superuser'
                          ? 'bg-tertiary-fixed-dim/20 border-tertiary-fixed-dim/40 text-tertiary-fixed-dim'
                          : op.role === 'admin'
                          ? 'bg-primary-fixed-dim/20 border-primary-fixed-dim/40 text-primary-fixed-dim'
                          : 'bg-surface-variant border-outline-variant/60 text-on-surface'"
                      >
                        {{ op.initials }}
                      </div>
                      <div>
                        <p class="font-bold text-on-surface text-sm">{{ op.full_name }}</p>
                        <p class="text-[11px] text-on-surface-variant font-mono">{{ op.email }}</p>
                      </div>
                    </div>
                  </td>

                  <!-- Username / UID -->
                  <td class="py-3 px-md">
                    <p class="text-sm text-on-surface font-mono font-semibold">{{ op.username }}</p>
                    <p class="text-[10px] font-mono text-on-surface-variant uppercase">{{ op.uid }}</p>
                  </td>

                  <!-- Role Badge with Icon -->
                  <td class="py-3 px-md">
                    <StatusBadge
                      :variant="op.role"
                      :icon="op.role === 'superuser' ? 'verified_user' : op.role === 'admin' ? 'admin_panel_settings' : op.role === 'operator' ? 'settings_accessibility' : 'visibility'"
                      size="sm"
                    >
                      {{ op.role === 'superuser' ? t('accessIdentity.superuser') : op.role === 'admin' ? t('accessIdentity.administrator') : op.role === 'operator' ? t('accessIdentity.operator') : t('accessIdentity.viewer') }}
                    </StatusBadge>
                  </td>

                  <!-- Status (Online / Offline) -->
                  <td class="py-3 px-md">
                    <StatusBadge
                      :variant="op.is_online ? 'online' : 'offline'"
                      :pulse="op.is_online"
                      :dot="true"
                      size="sm"
                    >
                      {{ op.is_online ? t('users.online') : t('users.offline') }}
                    </StatusBadge>
                  </td>

                  <!-- Action Buttons -->
                  <td class="py-3 px-md text-right">
                    <div class="flex items-center justify-end gap-1.5 text-on-surface-variant">
                      <button
                        type="button"
                        class="h-7 w-7 rounded-lg hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                        :title="t('users.editUser')"
                      >
                        <span class="material-symbols-outlined text-base">edit</span>
                      </button>
                      <button
                        type="button"
                        class="h-7 w-7 rounded-lg hover:text-primary-fixed-dim hover:bg-surface-variant/50 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                        :title="t('users.lockUser')"
                        @click="handleToggleLock(op)"
                      >
                        <span class="material-symbols-outlined text-base">{{ op.is_active ? 'lock' : 'lock_open' }}</span>
                      </button>
                      <button
                        type="button"
                        class="h-7 w-7 rounded-lg hover:text-error hover:bg-error-container/20 transition-colors flex items-center justify-center cursor-pointer active:scale-95"
                        :title="t('users.deleteUser')"
                        @click="handleDeleteUser(op.id)"
                      >
                        <span class="material-symbols-outlined text-base">delete</span>
                      </button>
                    </div>
                  </td>
                </tr>

                <tr v-if="filteredOperators.length === 0">
                  <td class="py-xl px-md text-center text-sm text-on-surface-variant" colspan="6">
                    {{ t('users.noUsersFound') }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </main>

    <!-- Modal: Add New User -->
    <BaseModal
      v-model="showAddModal"
      :title="t('users.addUserModalTitle')"
      icon="person_add"
      max-width="max-w-md"
    >
      <form id="addUserForm" @submit.prevent="handleCreateUser" class="flex flex-col gap-3">
        <BaseInput
          v-model="newUserForm.full_name"
          :label="t('users.fullName')"
          placeholder="e.g. Alex Morgan"
          size="sm"
        />

        <BaseInput
          v-model="newUserForm.username"
          :label="t('users.username')"
          placeholder="e.g. a.morgan"
          :required="true"
          size="sm"
        />

        <BaseInput
          v-model="newUserForm.email"
          :label="t('users.email')"
          placeholder="a.morgan@nms.local"
          type="email"
          size="sm"
        />

        <BaseInput
          v-model="newUserForm.password"
          :label="t('users.password')"
          placeholder="••••••••"
          type="password"
          :required="true"
          size="sm"
        />

        <BaseSelect
          v-model="newUserForm.role"
          :label="t('users.role')"
          :options="roleOptions"
          size="sm"
        />
      </form>

      <template #footer>
        <AppButton
          variant="ghost"
          size="sm"
          @click="showAddModal = false"
        >
          {{ t('users.cancel') }}
        </AppButton>
        <AppButton
          variant="primary"
          size="sm"
          type="submit"
          form="addUserForm"
          :loading="isSubmitting"
          @click="handleCreateUser"
        >
          {{ isSubmitting ? t('users.creating') : t('users.create') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Modal: Lock / Unlock User -->
    <ConfirmModal
      v-model="showLockModal"
      :title="selectedUserForAction?.is_active ? 'Блокировка пользователя' : 'Разблокировка пользователя'"
      :variant="selectedUserForAction?.is_active ? 'danger' : 'primary'"
      :confirm-text="selectedUserForAction?.is_active ? 'Заблокировать' : 'Разблокировать'"
      :cancel-text="t('common.cancel')"
      @confirm="confirmToggleLock"
    >
      <p v-if="selectedUserForAction">
        Вы действительно хотите {{ selectedUserForAction.is_active ? 'заблокировать' : 'разблокировать' }} оператора
        <strong class="text-primary-fixed-dim">{{ selectedUserForAction.full_name }}</strong> ({{ selectedUserForAction.username }})?
      </p>
    </ConfirmModal>
  </div>
</template>
