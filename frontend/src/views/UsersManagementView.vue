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
import { useAuthStore } from '@/stores/auth'
import type { User } from '@/api/auth'

const { t } = useI18n()
const authStore = useAuthStore()

interface OperatorItem {
  id: string
  uid: string
  username: string
  full_name: string
  email: string
  role: 'superuser' | 'admin' | 'operator' | 'viewer'
  is_online: boolean
  is_active: boolean
  must_change_password?: boolean
  initials: string
}

// State
const searchQuery = ref('')
const statusFilter = ref<'all' | 'online' | 'offline' | 'inactive' | 'superuser' | 'admin' | 'operator' | 'viewer'>('all')
const selectedUserIds = ref<string[]>([])
const copiedKey = ref<string | null>(null)
const sortKey = ref<'full_name' | 'username' | 'role' | 'is_online'>('full_name')
const sortOrder = ref<'asc' | 'desc'>('asc')
const loading = ref(false)

// Modals State
const showAddModal = ref(false)
const showEditModal = ref(false)
const showLockModal = ref(false)
const showDeleteModal = ref(false)
const showBulkDeleteModal = ref(false)
const isSubmitting = ref(false)
const formError = ref<string | null>(null)

// Target user for action
const selectedUserForAction = ref<OperatorItem | null>(null)
const userToDelete = ref<OperatorItem | null>(null)

// Form for new user
const newUserForm = ref({
  full_name: '',
  username: '',
  email: '',
  password: '',
  role: 'operator' as 'superuser' | 'admin' | 'operator' | 'viewer',
  must_change_password: true
})

// Form for editing user
const editUserForm = ref({
  id: '',
  full_name: '',
  username: '',
  email: '',
  role: 'operator' as 'superuser' | 'admin' | 'operator' | 'viewer',
  is_active: true,
  password: '',
  must_change_password: false
})

// Operators list
const operators = ref<OperatorItem[]>([])

async function loadUsers() {
  loading.value = true
  try {
    const list = await usersApi.list()
    if (list && Array.isArray(list)) {
      operators.value = list.map((u: User) => {
        const role = (u.roles && u.roles.includes('superuser')) || u.is_superuser ? 'superuser'
          : (u.roles && u.roles.includes('admin')) ? 'admin'
          : (u.roles && u.roles.includes('operator')) ? 'operator' : 'viewer'
        const initials = u.full_name
          ? u.full_name.split(' ').map((n: string) => n[0]).join('').slice(0, 2).toUpperCase()
          : u.username.slice(0, 2).toUpperCase()
        return {
          id: u.id,
          uid: u.id,
          username: u.username,
          full_name: u.full_name || u.username,
          email: u.email || `${u.username}@nms.local`,
          role,
          is_online: Boolean(u.is_active),
          is_active: Boolean(u.is_active),
          must_change_password: Boolean(u.must_change_password),
          initials
        }
      })
    }
  } catch (e) {
    console.warn('Failed to load users from API:', e)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  loadUsers()
})

// KPI Statistics
const totalCount = computed(() => operators.value.length)
const onlineCount = computed(() => operators.value.filter((op) => op.is_online).length)
const superuserCount = computed(() => operators.value.filter((op) => op.role === 'superuser').length)
const adminCount = computed(() => operators.value.filter((op) => op.role === 'superuser' || op.role === 'admin').length)
const inactiveCount = computed(() => operators.value.filter((op) => !op.is_active).length)

// Filtered and Sorted operators
const filteredOperators = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()

  const list = operators.value.filter((op) => {
    const matchesSearch = !q ||
      op.full_name.toLowerCase().includes(q) ||
      op.username.toLowerCase().includes(q) ||
      op.email.toLowerCase().includes(q) ||
      op.uid.toLowerCase().includes(q)

    if (!matchesSearch) return false

    if (statusFilter.value === 'all') return true
    if (statusFilter.value === 'online') return op.is_online
    if (statusFilter.value === 'offline') return !op.is_online
    if (statusFilter.value === 'inactive') return !op.is_active
    if (statusFilter.value === 'superuser') return op.role === 'superuser'
    if (statusFilter.value === 'admin') return op.role === 'admin'
    if (statusFilter.value === 'operator') return op.role === 'operator'
    if (statusFilter.value === 'viewer') return op.role === 'viewer'

    return true
  })

  // Sorting
  return [...list].sort((a, b) => {
    let aVal = a[sortKey.value]
    let bVal = b[sortKey.value]

    if (typeof aVal === 'boolean') {
      return sortOrder.value === 'asc'
        ? (aVal === bVal ? 0 : aVal ? -1 : 1)
        : (aVal === bVal ? 0 : aVal ? 1 : -1)
    }

    const cmp = String(aVal).localeCompare(String(bVal), undefined, { sensitivity: 'base' })
    return sortOrder.value === 'asc' ? cmp : -cmp
  })
})

function handleSort(key: 'full_name' | 'username' | 'role' | 'is_online') {
  if (sortKey.value === key) {
    sortOrder.value = sortOrder.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = key
    sortOrder.value = 'asc'
  }
}

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

function isProtectedUser(op: OperatorItem | null): boolean {
  if (!op) return false
  return op.role === 'superuser' || op.username === 'root' || op.id === 'ROOT-001'
}

function canEditUser(op: OperatorItem | null): boolean {
  if (!op) return false
  if (op.role === 'superuser' && !authStore.isSuperuser) {
    return false
  }
  return true
}

// Copy to clipboard helper
function copyToClipboard(text: string, key: string) {
  navigator.clipboard.writeText(text)
  copiedKey.value = key
  setTimeout(() => {
    if (copiedKey.value === key) {
      copiedKey.value = null
    }
  }, 2000)
}

// Export functions
function handleExportCsv(targetList = filteredOperators.value) {
  const header = ['ID', 'Full Name', 'Username', 'Email', 'Role', 'Status', 'Active']
  const rows = targetList.map((op) => [
    op.uid || op.id,
    `"${op.full_name}"`,
    op.username,
    op.email,
    op.role,
    op.is_online ? 'Online' : 'Offline',
    op.is_active ? 'Active' : 'Locked'
  ])
  const csvContent = 'data:text/csv;charset=utf-8,' + [header.join(','), ...rows.map((r) => r.join(','))].join('\n')
  const link = document.createElement('a')
  link.setAttribute('href', encodeURI(csvContent))
  link.setAttribute('download', `operators_${new Date().toISOString().slice(0, 10)}.csv`)
  document.body.appendChild(link)
  link.click()
  link.remove()
}

function handleExportJson(targetList = filteredOperators.value) {
  const dataStr = 'data:text/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(targetList, null, 2))
  const link = document.createElement('a')
  link.setAttribute('href', dataStr)
  link.setAttribute('download', `operators_${new Date().toISOString().slice(0, 10)}.json`)
  document.body.appendChild(link)
  link.click()
  link.remove()
}

// Create User
async function handleCreateUser() {
  if (!newUserForm.value.username.trim()) return
  isSubmitting.value = true
  try {
    const created = await usersApi.create({
      username: newUserForm.value.username.trim(),
      password: newUserForm.value.password.trim() || 'operator123',
      full_name: newUserForm.value.full_name.trim() || newUserForm.value.username.trim(),
      email: newUserForm.value.email.trim() || `${newUserForm.value.username.trim()}@nms.local`,
      roles: [newUserForm.value.role],
      is_active: true,
      must_change_password: newUserForm.value.must_change_password
    })

    const role = (created.roles && created.roles.includes('superuser')) ? 'superuser'
      : (created.roles && created.roles.includes('admin')) ? 'admin'
      : (created.roles && created.roles.includes('operator')) ? 'operator' : 'viewer'
    const initials = created.full_name
      ? created.full_name.split(' ').map((n: string) => n[0]).join('').slice(0, 2).toUpperCase()
      : created.username.slice(0, 2).toUpperCase()

    operators.value.unshift({
      id: created.id,
      uid: created.id,
      username: created.username,
      full_name: created.full_name || created.username,
      email: created.email || `${created.username}@nms.local`,
      role,
      is_online: true,
      is_active: true,
      must_change_password: Boolean(created.must_change_password),
      initials
    })

    newUserForm.value = {
      full_name: '',
      username: '',
      email: '',
      password: '',
      role: 'operator',
      must_change_password: true
    }
    showAddModal.value = false
  } catch (err: any) {
    console.error('Failed to create user via API, falling back to local creation:', err)
    const id = `UID-${Math.random().toString(36).substring(2, 8).toUpperCase()}`
    const rawUid = `${Math.random().toString(36).substring(2, 10)}-${Math.random().toString(36).substring(2, 6)}-4966-9105-${Math.random().toString(36).substring(2, 12)}`
    const initials = newUserForm.value.full_name
      ? newUserForm.value.full_name.split(' ').map((n) => n[0]).join('').slice(0, 2).toUpperCase()
      : newUserForm.value.username.slice(0, 2).toUpperCase()

    operators.value.unshift({
      id,
      uid: rawUid,
      username: newUserForm.value.username.trim(),
      full_name: newUserForm.value.full_name.trim() || newUserForm.value.username.trim(),
      email: newUserForm.value.email.trim() || `${newUserForm.value.username.trim()}@nms.local`,
      role: newUserForm.value.role,
      is_online: true,
      is_active: true,
      must_change_password: newUserForm.value.must_change_password,
      initials
    })

    newUserForm.value = {
      full_name: '',
      username: '',
      email: '',
      password: '',
      role: 'operator',
      must_change_password: true
    }
    showAddModal.value = false
  } finally {
    isSubmitting.value = false
  }
}

// Edit User
function handleOpenEdit(op: OperatorItem) {
  if (!canEditUser(op)) return
  editUserForm.value = {
    id: op.id,
    full_name: op.full_name,
    username: op.username,
    email: op.email,
    role: op.role,
    is_active: op.is_active,
    password: '',
    must_change_password: op.must_change_password ?? false
  }
  showEditModal.value = true
}

async function handleSaveEdit() {
  if (!editUserForm.value.username.trim()) return
  isSubmitting.value = true
  try {
    await usersApi.update(editUserForm.value.id, {
      full_name: editUserForm.value.full_name.trim(),
      email: editUserForm.value.email.trim(),
      is_active: editUserForm.value.is_active,
      roles: [editUserForm.value.role],
      must_change_password: editUserForm.value.must_change_password,
      ...(editUserForm.value.password.trim() ? { password: editUserForm.value.password.trim() } : {})
    })
  } catch (err) {
    console.warn('Backend update failed or offline, updating locally:', err)
  } finally {
    const index = operators.value.findIndex((op) => op.id === editUserForm.value.id)
    if (index >= 0) {
      const initials = editUserForm.value.full_name
        ? editUserForm.value.full_name.split(' ').map((n) => n[0]).join('').slice(0, 2).toUpperCase()
        : editUserForm.value.username.slice(0, 2).toUpperCase()

      operators.value[index] = {
        ...operators.value[index],
        full_name: editUserForm.value.full_name.trim() || editUserForm.value.username.trim(),
        username: editUserForm.value.username.trim(),
        email: editUserForm.value.email.trim(),
        role: editUserForm.value.role,
        is_active: editUserForm.value.is_active,
        is_online: editUserForm.value.is_active ? operators.value[index].is_online : false,
        must_change_password: editUserForm.value.must_change_password,
        initials
      }
    }
    showEditModal.value = false
    isSubmitting.value = false
  }
}

// Delete User
function promptDeleteUser(op: OperatorItem) {
  if (isProtectedUser(op)) return
  userToDelete.value = op
  showDeleteModal.value = true
}

async function confirmDeleteUser() {
  if (userToDelete.value) {
    const id = userToDelete.value.id
    try {
      await usersApi.delete(id)
    } catch (err) {
      console.warn('Backend delete failed, removing locally:', err)
    }
    operators.value = operators.value.filter((op) => op.id !== id)
    selectedUserIds.value = selectedUserIds.value.filter((uid) => uid !== id)
  }
  showDeleteModal.value = false
  userToDelete.value = null
}

// Toggle Lock
function handleToggleLock(op: OperatorItem) {
  if (isProtectedUser(op)) return
  selectedUserForAction.value = op
  showLockModal.value = true
}

async function confirmToggleLock() {
  if (selectedUserForAction.value) {
    const newActiveState = !selectedUserForAction.value.is_active
    try {
      await usersApi.update(selectedUserForAction.value.id, {
        is_active: newActiveState
      })
    } catch (err) {
      console.warn('Backend lock toggle failed, updating locally:', err)
    }
    selectedUserForAction.value.is_active = newActiveState
    selectedUserForAction.value.is_online = newActiveState
  }
  showLockModal.value = false
}

// Bulk Actions
async function handleBulkLock(lockState: boolean) {
  for (const op of operators.value) {
    if (selectedUserIds.value.includes(op.id) && !isProtectedUser(op)) {
      op.is_active = lockState
      if (!lockState) op.is_online = false
      try {
        await usersApi.update(op.id, { is_active: lockState })
      } catch (err) {
        console.warn(`Failed to update user ${op.id}:`, err)
      }
    }
  }
}

function handleBulkExport(format: 'csv' | 'json') {
  const selectedList = operators.value.filter((op) => selectedUserIds.value.includes(op.id))
  if (format === 'csv') handleExportCsv(selectedList)
  else handleExportJson(selectedList)
}

function promptBulkDelete() {
  showBulkDeleteModal.value = true
}

async function confirmBulkDelete() {
  const idsToDelete = [...selectedUserIds.value]
  for (const id of idsToDelete) {
    const op = operators.value.find((u) => u.id === id)
    if (op && !isProtectedUser(op)) {
      try {
        await usersApi.delete(id)
      } catch (err) {
        console.warn(`Failed to delete user ${id}:`, err)
      }
    }
  }
  operators.value = operators.value.filter((op) => !(selectedUserIds.value.includes(op.id) && !isProtectedUser(op)))
  selectedUserIds.value = []
  showBulkDeleteModal.value = false
}

const statusOptions = computed(() => [
  { value: 'all', label: t('users.allStatuses') },
  { value: 'online', label: t('users.onlineOnly') },
  { value: 'offline', label: t('users.offlineOnly') },
  { value: 'inactive', label: t('users.inactiveStatus') },
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

const createRoleOptions = computed(() => {
  const opts = []
  if (authStore.isSuperuser) {
    opts.push({
      value: 'superuser',
      label: superuserCount.value >= 4
        ? `${t('accessIdentity.superuser')} (Лимит 4)`
        : t('accessIdentity.superuser'),
      disabled: superuserCount.value >= 4
    })
  }
  opts.push(
    { value: 'admin', label: t('accessIdentity.administrator') },
    { value: 'operator', label: t('accessIdentity.operator') },
    { value: 'viewer', label: t('accessIdentity.viewer') }
  )
  return opts
})

const editRoleOptions = computed(() => {
  const opts = []
  const isTargetSuper = editUserForm.value.role === 'superuser'
  if (authStore.isSuperuser) {
    if (isTargetSuper) {
      opts.push({
        value: 'superuser',
        label: t('accessIdentity.superuser'),
        disabled: superuserCount.value <= 1
      })
    } else {
      opts.push({
        value: 'superuser',
        label: superuserCount.value >= 4
          ? `${t('accessIdentity.superuser')} (Лимит 4)`
          : t('accessIdentity.superuser'),
        disabled: superuserCount.value >= 4
      })
    }
  }
  opts.push(
    { value: 'admin', label: t('accessIdentity.administrator') },
    { value: 'operator', label: t('accessIdentity.operator') },
    { value: 'viewer', label: t('accessIdentity.viewer') }
  )
  return opts
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
          :title="t('users.title')"
          :subtitle="t('users.subtitle')"
          icon="manage_accounts"
        />

        <!-- KPI Summary Cards -->
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-md">
          <!-- Total Operators -->
          <div class="bg-surface-container-low border border-outline-variant/60 rounded-xl p-4 flex items-center justify-between shadow-card-dark transition-all hover:border-outline-variant">
            <div class="flex flex-col gap-1">
              <span class="text-xs text-on-surface-variant font-medium">{{ t('users.statsTotal') }}</span>
              <span class="text-2xl font-bold font-mono text-on-surface">{{ totalCount }}</span>
            </div>
            <div class="w-10 h-10 rounded-lg bg-surface-variant/40 border border-outline-variant/60 flex items-center justify-center text-primary-fixed-dim">
              <span class="material-symbols-outlined text-xl">group</span>
            </div>
          </div>

          <!-- Online Now -->
          <div class="bg-surface-container-low border border-outline-variant/60 rounded-xl p-4 flex items-center justify-between shadow-card-dark transition-all hover:border-outline-variant">
            <div class="flex flex-col gap-1">
              <span class="text-xs text-on-surface-variant font-medium">{{ t('users.statsOnline') }}</span>
              <div class="flex items-center gap-2">
                <span class="text-2xl font-bold font-mono text-emerald-600 dark:text-emerald-400">{{ onlineCount }}</span>
                <span class="relative flex h-2.5 w-2.5">
                  <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                  <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-emerald-500"></span>
                </span>
              </div>
            </div>
            <div class="w-10 h-10 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-600 dark:text-emerald-400">
              <span class="material-symbols-outlined text-xl">wifi_tethering</span>
            </div>
          </div>

          <!-- Admins & Superusers -->
          <div class="bg-surface-container-low border border-outline-variant/60 rounded-xl p-4 flex items-center justify-between shadow-card-dark transition-all hover:border-outline-variant">
            <div class="flex flex-col gap-1">
              <span class="text-xs text-on-surface-variant font-medium">{{ t('users.statsAdmins') }}</span>
              <span class="text-2xl font-bold font-mono text-primary-fixed-dim">{{ adminCount }}</span>
            </div>
            <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim">
              <span class="material-symbols-outlined text-xl">shield_person</span>
            </div>
          </div>

          <!-- Locked / Inactive -->
          <div class="bg-surface-container-low border border-outline-variant/60 rounded-xl p-4 flex items-center justify-between shadow-card-dark transition-all hover:border-outline-variant">
            <div class="flex flex-col gap-1">
              <span class="text-xs text-on-surface-variant font-medium">{{ t('users.statsInactive') }}</span>
              <span class="text-2xl font-bold font-mono" :class="inactiveCount > 0 ? 'text-amber-600 dark:text-amber-400' : 'text-on-surface-variant'">
                {{ inactiveCount }}
              </span>
            </div>
            <div class="w-10 h-10 rounded-lg bg-surface-variant/40 border border-outline-variant/60 flex items-center justify-center text-on-surface-variant">
              <span class="material-symbols-outlined text-xl">person_off</span>
            </div>
          </div>
        </div>

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

            <!-- Active Operators Counter Badge -->
            <div class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-variant/40 border border-outline-variant/40 text-[11px] font-mono text-on-surface-variant hidden xl:flex">
              <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
              <span>{{ t('users.showingActiveOperators', { count: onlineCount }) }}</span>
            </div>
          </div>

          <!-- Actions: Export Dropdown / Add User -->
          <div class="flex items-center gap-2 flex-wrap">
            <!-- Export Buttons -->
            <div class="flex items-center bg-surface-container-lowest rounded-lg border border-outline-variant/60 overflow-hidden">
              <button
                type="button"
                @click="handleExportCsv()"
                class="px-3 py-1.5 text-xs font-semibold text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-colors flex items-center gap-1 border-r border-outline-variant/60 cursor-pointer"
                :title="t('users.exportCsv')"
              >
                <span class="material-symbols-outlined text-sm">download</span>
                CSV
              </button>
              <button
                type="button"
                @click="handleExportJson()"
                class="px-3 py-1.5 text-xs font-semibold text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-colors flex items-center gap-1 cursor-pointer"
                :title="t('users.exportJson')"
              >
                JSON
              </button>
            </div>

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

        <!-- Floating Bulk Actions Bar (when >= 1 user selected) -->
        <transition
          enter-active-class="transition duration-200 ease-out"
          enter-from-class="transform -translate-y-2 opacity-0"
          enter-to-class="transform translate-y-0 opacity-100"
          leave-active-class="transition duration-150 ease-in"
          leave-from-class="transform translate-y-0 opacity-100"
          leave-to-class="transform -translate-y-2 opacity-0"
        >
          <div
            v-if="selectedUserIds.length > 0"
            class="flex items-center justify-between flex-wrap gap-md bg-surface-container-high border border-primary-fixed-dim/40 px-4 py-2.5 rounded-xl shadow-glow-primary-sm"
          >
            <div class="flex items-center gap-3">
              <span class="flex h-2 w-2 rounded-full bg-primary-fixed-dim"></span>
              <span class="text-xs font-bold text-on-surface font-mono">
                {{ t('users.selectedCount', { count: selectedUserIds.length }) }}
              </span>
            </div>

            <div class="flex items-center gap-2 flex-wrap">
              <AppButton
                variant="outline"
                size="sm"
                icon="lock"
                @click="handleBulkLock(false)"
              >
                {{ t('users.bulkLock') }}
              </AppButton>
              <AppButton
                variant="outline"
                size="sm"
                icon="lock_open"
                @click="handleBulkLock(true)"
              >
                {{ t('users.bulkUnlock') }}
              </AppButton>
              <AppButton
                variant="outline"
                size="sm"
                icon="download"
                @click="handleBulkExport('csv')"
              >
                {{ t('users.bulkExportCsv') }}
              </AppButton>
              <AppButton
                variant="danger"
                size="sm"
                icon="delete"
                @click="promptBulkDelete"
              >
                {{ t('users.bulkDelete') }}
              </AppButton>
              <button
                type="button"
                @click="selectedUserIds = []"
                class="text-xs text-on-surface-variant hover:text-on-surface ml-2 underline cursor-pointer"
              >
                {{ t('users.clearSelection') }}
              </button>
            </div>
          </div>
        </transition>

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
                  <th
                    class="py-3 px-md cursor-pointer hover:text-on-surface transition-colors select-none"
                    @click="handleSort('full_name')"
                  >
                    <div class="flex items-center gap-1">
                      <span>{{ t('users.userCol') }}</span>
                      <span v-if="sortKey === 'full_name'" class="material-symbols-outlined text-xs">
                        {{ sortOrder === 'asc' ? 'arrow_upward' : 'arrow_downward' }}
                      </span>
                    </div>
                  </th>
                  <th
                    class="py-3 px-md cursor-pointer hover:text-on-surface transition-colors select-none"
                    @click="handleSort('username')"
                  >
                    <div class="flex items-center gap-1">
                      <span>{{ t('users.usernameIdCol') }}</span>
                      <span v-if="sortKey === 'username'" class="material-symbols-outlined text-xs">
                        {{ sortOrder === 'asc' ? 'arrow_upward' : 'arrow_downward' }}
                      </span>
                    </div>
                  </th>
                  <th
                    class="py-3 px-md cursor-pointer hover:text-on-surface transition-colors select-none"
                    @click="handleSort('role')"
                  >
                    <div class="flex items-center gap-1">
                      <span>{{ t('users.roleCol') }}</span>
                      <span v-if="sortKey === 'role'" class="material-symbols-outlined text-xs">
                        {{ sortOrder === 'asc' ? 'arrow_upward' : 'arrow_downward' }}
                      </span>
                    </div>
                  </th>
                  <th
                    class="py-3 px-md cursor-pointer hover:text-on-surface transition-colors select-none"
                    @click="handleSort('is_online')"
                  >
                    <div class="flex items-center gap-1">
                      <span>{{ t('users.statusCol') }}</span>
                      <span v-if="sortKey === 'is_online'" class="material-symbols-outlined text-xs">
                        {{ sortOrder === 'asc' ? 'arrow_upward' : 'arrow_downward' }}
                      </span>
                    </div>
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
                      <!-- Avatar with status indicator dot -->
                      <div class="relative shrink-0">
                        <div
                          class="w-10 h-10 rounded-lg flex items-center justify-center font-bold font-mono border"
                          :class="op.role === 'superuser'
                            ? 'bg-tertiary-fixed-dim/20 border-tertiary-fixed-dim/40 text-tertiary-fixed-dim'
                            : op.role === 'admin'
                            ? 'bg-primary-fixed-dim/20 border-primary-fixed-dim/40 text-primary-fixed-dim'
                            : 'bg-surface-variant border-outline-variant/60 text-on-surface'"
                        >
                          {{ op.initials }}
                        </div>
                        <span
                          class="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-surface-container-low"
                          :class="op.is_online ? 'bg-emerald-500' : 'bg-outline-variant'"
                        ></span>
                      </div>

                      <div class="flex flex-col">
                        <div class="flex items-center gap-2">
                          <p class="font-bold text-on-surface text-sm">{{ op.full_name }}</p>
                          <span
                            v-if="!op.is_active"
                            class="text-[10px] px-1.5 py-0.5 rounded bg-error-container/40 text-error font-medium"
                          >
                            {{ t('users.inactiveStatus') }}
                          </span>
                        </div>
                        <button
                          type="button"
                          @click="copyToClipboard(op.email, `email-${op.id}`)"
                          class="text-[11px] text-on-surface-variant font-mono hover:text-primary-fixed-dim transition-colors flex items-center gap-1 text-left cursor-pointer group"
                          :title="t('users.copyEmail')"
                        >
                          <span>{{ op.email }}</span>
                          <span class="material-symbols-outlined text-[12px] opacity-0 group-hover:opacity-100 transition-opacity">content_copy</span>
                          <span v-if="copiedKey === `email-${op.id}`" class="text-[10px] text-emerald-600 dark:text-emerald-400 font-bold ml-1">
                            {{ t('users.copied') }}
                          </span>
                        </button>
                      </div>
                    </div>
                  </td>

                  <!-- Username / UID -->
                  <td class="py-3 px-md">
                    <p class="text-sm text-on-surface font-mono font-semibold">{{ op.username }}</p>
                    <button
                      type="button"
                      @click="copyToClipboard(op.uid || op.id, `uid-${op.id}`)"
                      class="text-[10px] font-mono text-on-surface-variant uppercase hover:text-primary-fixed-dim transition-colors flex items-center gap-1 cursor-pointer group"
                      :title="t('users.copyUid')"
                    >
                      <span>UID: {{ (op.uid || op.id).slice(0, 18) }}...</span>
                      <span class="material-symbols-outlined text-[11px] opacity-0 group-hover:opacity-100 transition-opacity">content_copy</span>
                      <span v-if="copiedKey === `uid-${op.id}`" class="text-[10px] text-emerald-600 dark:text-emerald-400 font-bold ml-1">
                        {{ t('users.copied') }}
                      </span>
                    </button>
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
                      <!-- Edit User Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="canEditUser(op)
                          ? 'hover:text-primary-fixed-dim hover:bg-surface-variant/50 cursor-pointer text-on-surface-variant'
                          : 'opacity-30 cursor-not-allowed text-on-surface-variant'"
                        :title="canEditUser(op) ? t('users.editUser') : t('users.protectedRoot')"
                        :disabled="!canEditUser(op)"
                        @click="handleOpenEdit(op)"
                      >
                        <span class="material-symbols-outlined text-base">edit</span>
                      </button>

                      <!-- Lock / Unlock Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="isProtectedUser(op)
                          ? 'opacity-30 cursor-not-allowed text-on-surface-variant'
                          : 'hover:text-amber-600 dark:hover:text-amber-400 hover:bg-amber-400/10 cursor-pointer text-on-surface-variant'"
                        :title="isProtectedUser(op) ? t('users.protectedRoot') : t('users.lockUser')"
                        :disabled="isProtectedUser(op)"
                        @click="handleToggleLock(op)"
                      >
                        <span class="material-symbols-outlined text-base">{{ op.is_active ? 'lock' : 'lock_open' }}</span>
                      </button>

                      <!-- Delete User Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="isProtectedUser(op)
                          ? 'opacity-30 cursor-not-allowed text-on-surface-variant'
                          : 'hover:text-error hover:bg-error-container/20 cursor-pointer text-on-surface-variant'"
                        :title="isProtectedUser(op) ? t('users.protectedRoot') : t('users.deleteUser')"
                        :disabled="isProtectedUser(op)"
                        @click="promptDeleteUser(op)"
                      >
                        <span class="material-symbols-outlined text-base">delete</span>
                      </button>
                    </div>
                  </td>
                </tr>

                <tr v-if="loading && filteredOperators.length === 0">
                  <td class="py-xl px-md text-center text-sm text-on-surface-variant" colspan="6">
                    <div class="flex flex-col items-center justify-center gap-2 py-6">
                      <span class="material-symbols-outlined text-3xl text-primary-fixed-dim animate-spin">progress_activity</span>
                      <p>{{ t('common.loading') }}</p>
                    </div>
                  </td>
                </tr>

                <tr v-else-if="filteredOperators.length === 0">
                  <td class="py-xl px-md text-center text-sm text-on-surface-variant" colspan="6">
                    <div class="flex flex-col items-center justify-center gap-2 py-6">
                      <span class="material-symbols-outlined text-3xl text-outline-variant">person_search</span>
                      <p>{{ t('users.noUsersFound') }}</p>
                    </div>
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
          :options="createRoleOptions"
          size="sm"
        />

        <label class="flex items-center gap-2 pt-1 cursor-pointer select-none">
          <input
            v-model="newUserForm.must_change_password"
            type="checkbox"
            class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
          />
          <span class="text-xs text-on-surface">
            {{ t('accessIdentity.mandatoryPasswordChange') }}
          </span>
        </label>
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

    <!-- Modal: Edit User -->
    <BaseModal
      v-model="showEditModal"
      :title="t('users.editUserModalTitle')"
      icon="edit"
      max-width="max-w-md"
    >
      <form id="editUserForm" @submit.prevent="handleSaveEdit" class="flex flex-col gap-3">
        <BaseInput
          v-model="editUserForm.full_name"
          :label="t('users.fullName')"
          placeholder="e.g. Alex Morgan"
          size="sm"
        />

        <BaseInput
          v-model="editUserForm.username"
          :label="t('users.username')"
          :disabled="editUserForm.username === 'root'"
          size="sm"
        />

        <BaseInput
          v-model="editUserForm.email"
          :label="t('users.email')"
          type="email"
          size="sm"
        />

        <BaseSelect
          v-model="editUserForm.role"
          :label="t('users.role')"
          :options="editRoleOptions"
          :disabled="editUserForm.username === 'root' || (editUserForm.role === 'superuser' && superuserCount <= 1)"
          size="sm"
        />

        <BaseInput
          v-model="editUserForm.password"
          :label="t('users.newPasswordOptional')"
          placeholder="••••••••"
          type="password"
          size="sm"
        />

        <label class="flex items-center gap-2 pt-1 cursor-pointer select-none">
          <input
            v-model="editUserForm.must_change_password"
            type="checkbox"
            class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
          />
          <span class="text-xs text-on-surface">
            {{ t('accessIdentity.mandatoryPasswordChange') }}
          </span>
        </label>
      </form>

      <template #footer>
        <AppButton
          variant="ghost"
          size="sm"
          @click="showEditModal = false"
        >
          {{ t('users.cancel') }}
        </AppButton>
        <AppButton
          variant="primary"
          size="sm"
          type="submit"
          form="editUserForm"
          :loading="isSubmitting"
          @click="handleSaveEdit"
        >
          {{ isSubmitting ? t('users.saving') : t('users.save') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Modal: Lock / Unlock User -->
    <ConfirmModal
      v-model="showLockModal"
      :title="selectedUserForAction?.is_active ? 'Блокировка пользователя' : 'Разблокировка пользователя'"
      :variant="selectedUserForAction?.is_active ? 'danger' : 'primary'"
      :confirm-text="selectedUserForAction?.is_active ? 'Заблокировать' : 'Разблокировать'"
      :cancel-text="t('users.cancel')"
      @confirm="confirmToggleLock"
    >
      <p v-if="selectedUserForAction">
        Вы действительно хотите {{ selectedUserForAction.is_active ? 'заблокировать' : 'разблокировать' }} оператора
        <strong class="text-primary-fixed-dim">{{ selectedUserForAction.full_name }}</strong> ({{ selectedUserForAction.username }})?
      </p>
    </ConfirmModal>

    <!-- Modal: Delete Single User Confirmation -->
    <ConfirmModal
      v-model="showDeleteModal"
      :title="t('users.deleteConfirmTitle')"
      variant="danger"
      :confirm-text="t('users.deleteUser')"
      :cancel-text="t('users.cancel')"
      icon="delete"
      @confirm="confirmDeleteUser"
    >
      <p v-if="userToDelete">
        {{ t('users.deleteConfirmMessage', { name: userToDelete.full_name || userToDelete.username }) }}
      </p>
    </ConfirmModal>

    <!-- Modal: Bulk Delete Confirmation -->
    <ConfirmModal
      v-model="showBulkDeleteModal"
      :title="t('users.deleteBulkConfirmTitle')"
      variant="danger"
      :confirm-text="t('users.bulkDelete')"
      :cancel-text="t('users.cancel')"
      icon="delete_forever"
      @confirm="confirmBulkDelete"
    >
      <p>
        {{ t('users.deleteBulkConfirmMessage', { count: selectedUserIds.length }) }}
      </p>
    </ConfirmModal>
  </div>
</template>
