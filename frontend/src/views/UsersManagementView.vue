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
import { settingsApi, type SecurityPolicies } from '@/api/settings'
import { useAuthStore } from '@/stores/auth'
import type { User } from '@/api/auth'
import { getUserInitials } from '@/utils/user'
import { useToast } from '@/composables/useToast'

const { t } = useI18n()
const authStore = useAuthStore()
const toast = useToast()

interface OperatorItem {
  id: string
  uid: string
  username: string
  full_name: string
  email: string
  department?: string
  role: 'superuser' | 'admin' | 'operator' | 'viewer'
  is_online: boolean
  is_active: boolean
  must_change_password?: boolean
  is_totp_enabled?: boolean
  force_2fa?: boolean | null
  initials: string
  avatar?: string | null
}

function getOperatorAvatar(op: OperatorItem): string | null {
  if (authStore.user && (op.id === authStore.user.id || op.username === authStore.user.username)) {
    return authStore.avatar || op.avatar || null
  }
  return op.avatar || null
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
  department: '',
  password: '',
  role: 'operator' as 'superuser' | 'admin' | 'operator' | 'viewer',
  force_2fa: 'default' as 'default' | 'enforce' | 'exempt',
  must_change_password: true
})

// Form for editing user (role and password reset only)
const editUserForm = ref({
  id: '',
  full_name: '',
  username: '',
  role: 'operator' as 'superuser' | 'admin' | 'operator' | 'viewer',
  force_2fa: 'default' as 'default' | 'enforce' | 'exempt',
  password: '',
  must_change_password: true
})

const mfaEnforceOptions = computed(() => [
  { value: 'default', label: t('users.mfaPolicyDefault') },
  { value: 'enforce', label: t('users.mfaPolicyEnforced') },
  { value: 'exempt', label: t('users.mfaPolicyExempt') }
])

function getMfaBadgeInfo(op: OperatorItem) {
  if (op.is_totp_enabled) {
    return {
      label: t('users.mfaStatusActive'),
      colorClass: 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/30',
      icon: 'verified_user'
    }
  }
  if (op.force_2fa === false) {
    return {
      label: t('users.mfaStatusExempt'),
      colorClass: 'bg-surface-variant/40 text-on-surface-variant border border-outline-variant/40',
      icon: 'remove_moderator'
    }
  }
  if (op.force_2fa === true) {
    return {
      label: t('users.mfaStatusRequired'),
      colorClass: 'bg-amber-500/10 text-amber-400 border border-amber-500/30',
      icon: 'warning'
    }
  }
  const policy = securityPolicies.value
  const scope = policy?.mfa_scope || (policy?.force_2fa ? 'all' : 'disabled')
  const isEnforced = scope === 'all' || (scope === 'admins_only' && (op.role === 'admin' || op.role === 'superuser'))
  if (isEnforced) {
    return {
      label: t('users.mfaStatusRequired'),
      colorClass: 'bg-amber-500/10 text-amber-400 border border-amber-500/30',
      icon: 'warning'
    }
  }
  return {
    label: t('users.mfaStatusOptional'),
    colorClass: 'bg-surface-variant/40 text-on-surface-variant border border-outline-variant/40',
    icon: 'lock_open'
  }
}

// Operators list & Security Policies
const operators = ref<OperatorItem[]>([])
const securityPolicies = ref<SecurityPolicies | null>(null)

const passwordHintText = computed(() => {
  const policy = securityPolicies.value
  const reqs: string[] = []
  if (policy) {
    if (policy.min_password_length) {
      reqs.push(t('users.reqMinLength', { count: policy.min_password_length }))
    }
    if (policy.require_uppercase) {
      reqs.push(t('users.reqUppercase'))
    }
    if (policy.require_digits) {
      reqs.push(t('users.reqDigits'))
    }
    if (policy.require_special) {
      reqs.push(t('users.reqSpecial'))
    }
  } else {
    reqs.push(t('users.reqMinLength', { count: 8 }), t('users.reqUppercase'), t('users.reqDigits'), t('users.reqSpecial'))
  }
  const reqStr = reqs.join(', ')
  return t('users.passwordRequirements', { requirements: reqStr })
})

const editPasswordHintText = computed(() => {
  const policy = securityPolicies.value
  const reqs: string[] = []
  if (policy) {
    if (policy.min_password_length) {
      reqs.push(t('users.reqMinLength', { count: policy.min_password_length }))
    }
    if (policy.require_uppercase) {
      reqs.push(t('users.reqUppercase'))
    }
    if (policy.require_digits) {
      reqs.push(t('users.reqDigits'))
    }
    if (policy.require_special) {
      reqs.push(t('users.reqSpecial'))
    }
  } else {
    reqs.push(t('users.reqMinLength', { count: 8 }), t('users.reqUppercase'), t('users.reqDigits'), t('users.reqSpecial'))
  }
  const reqStr = reqs.join(', ')
  return t('users.editPasswordRequirements', { requirements: reqStr })
})

async function loadUsers() {
  loading.value = true
  try {
    const [listRes, policiesRes] = await Promise.allSettled([
      usersApi.list(),
      settingsApi.getSecurityPolicies()
    ])

    if (listRes.status === 'fulfilled' && Array.isArray(listRes.value)) {
      operators.value = listRes.value.map((u: User) => {
        const role = (u.roles && u.roles.includes('superuser')) || u.is_superuser ? 'superuser'
          : (u.roles && u.roles.includes('admin')) ? 'admin'
          : (u.roles && u.roles.includes('operator')) ? 'operator' : 'viewer'
        const initials = getUserInitials(u.full_name, u.username)
        return {
          id: u.id,
          uid: u.id,
          username: u.username,
          full_name: u.full_name || u.username,
          email: u.email || `${u.username}@aethercore.local`,
          department: u.department,
          role,
          is_online: Boolean(u.is_online),
          is_active: Boolean(u.is_active),
          must_change_password: Boolean(u.must_change_password),
          is_totp_enabled: Boolean(u.is_totp_enabled),
          force_2fa: u.force_2fa,
          initials
        }
      })
    }

    if (policiesRes.status === 'fulfilled' && policiesRes.value) {
      securityPolicies.value = policiesRes.value
    }
  } catch (e) {
    console.warn('Failed to load users or security policies from API:', e)
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

function getRoleLevel(role: string): number {
  if (role === 'superuser') return 4
  if (role === 'admin') return 3
  if (role === 'operator') return 2
  return 1
}

function isProtectedUser(op: OperatorItem | null): boolean {
  if (!op) return false
  return op.role === 'superuser' || op.username === 'root' || op.id === 'ROOT-001'
}

function canEditUser(op: OperatorItem | null): boolean {
  if (!op) return false
  if (!authStore.canManageUsers) return false
  if (op.role === 'superuser' && !authStore.isSuperuser) return false
  if (getRoleLevel(op.role) > authStore.currentUserRoleLevel) return false
  return true
}

function canLockUser(op: OperatorItem | null): boolean {
  if (!op) return false
  if (!authStore.canManageUsers) return false
  if (isProtectedUser(op)) return false
  if (getRoleLevel(op.role) >= authStore.currentUserRoleLevel && !authStore.isSuperuser) return false
  return true
}

function canDeleteUser(op: OperatorItem | null): boolean {
  if (!op) return false
  if (!authStore.canManageUsers) return false
  if (isProtectedUser(op)) return false
  if (getRoleLevel(op.role) >= authStore.currentUserRoleLevel && !authStore.isSuperuser) return false
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

const editFormError = ref<string | null>(null)

function openAddModal() {
  formError.value = null
  newUserForm.value = {
    full_name: '',
    username: '',
    email: '',
    department: '',
    password: '',
    role: 'operator',
    force_2fa: 'default',
    must_change_password: true
  }
  showAddModal.value = true
}

// Create User
async function handleCreateUser() {
  if (isSubmitting.value) return
  if (!newUserForm.value.username.trim()) {
    formError.value = t('users.usernameRequired')
    return
  }
  isSubmitting.value = true
  formError.value = null
  try {
    const isSuper = newUserForm.value.role === 'superuser'
    const force2faVal = newUserForm.value.force_2fa === 'enforce' ? true : newUserForm.value.force_2fa === 'exempt' ? false : null
    const created = await usersApi.create({
      username: newUserForm.value.username.trim(),
      password: newUserForm.value.password.trim() || 'Operator123!',
      full_name: newUserForm.value.full_name.trim() || newUserForm.value.username.trim(),
      email: newUserForm.value.email.trim() || `${newUserForm.value.username.trim()}@aethercore.local`,
      department: newUserForm.value.department.trim() || undefined,
      roles: [newUserForm.value.role],
      is_superuser: isSuper,
      is_active: true,
      force_2fa: force2faVal,
      must_change_password: newUserForm.value.must_change_password
    })

    const role = (created.roles && created.roles.includes('superuser')) || created.is_superuser ? 'superuser'
      : (created.roles && created.roles.includes('admin')) ? 'admin'
      : (created.roles && created.roles.includes('operator')) ? 'operator' : 'viewer'
    const initials = getUserInitials(created.full_name, created.username)

    operators.value.unshift({
      id: created.id,
      uid: created.id,
      username: created.username,
      full_name: created.full_name || created.username,
      email: created.email || `${created.username}@aethercore.local`,
      department: created.department,
      role,
      is_online: Boolean(created.is_online),
      is_active: true,
      must_change_password: Boolean(created.must_change_password),
      is_totp_enabled: false,
      force_2fa: force2faVal,
      initials
    })

    toast.success(t('users.createSuccess') || t('common.changesApplied'))
    showAddModal.value = false
  } catch (err: any) {
    console.error('Failed to create user via API:', err)
    formError.value = err.message || t('users.createError')
    toast.error(formError.value || t('users.createError'))
  } finally {
    isSubmitting.value = false
  }
}

// Edit User (Role, 2FA override and Password reset)
function handleOpenEdit(op: OperatorItem) {
  if (!canEditUser(op)) return
  editFormError.value = null
  editUserForm.value = {
    id: op.id,
    full_name: op.full_name,
    username: op.username,
    role: op.role,
    force_2fa: op.force_2fa === true ? 'enforce' : op.force_2fa === false ? 'exempt' : 'default',
    password: '',
    must_change_password: true
  }
  showEditModal.value = true
}

async function handleSaveEdit() {
  if (isSubmitting.value) return
  isSubmitting.value = true
  editFormError.value = null
  try {
    const isSuper = editUserForm.value.role === 'superuser'
    const force2faVal = editUserForm.value.force_2fa === 'enforce' ? true : editUserForm.value.force_2fa === 'exempt' ? false : null
    await usersApi.update(editUserForm.value.id, {
      roles: [editUserForm.value.role],
      is_superuser: isSuper,
      force_2fa: force2faVal,
      must_change_password: editUserForm.value.password.trim() ? editUserForm.value.must_change_password : undefined,
      ...(editUserForm.value.password.trim() ? { password: editUserForm.value.password.trim() } : {})
    })
    const index = operators.value.findIndex((op) => op.id === editUserForm.value.id)
    if (index >= 0) {
      operators.value[index] = {
        ...operators.value[index],
        role: editUserForm.value.role,
        force_2fa: force2faVal,
        must_change_password: editUserForm.value.password.trim() ? editUserForm.value.must_change_password : operators.value[index].must_change_password
      }
    }
    toast.success(t('users.updateSuccess') || t('common.changesApplied'))
    showEditModal.value = false
  } catch (err: any) {
    console.error('Backend update failed:', err)
    editFormError.value = err.message || t('users.updateError')
    toast.error(editFormError.value || t('users.updateError'))
  } finally {
    isSubmitting.value = false
  }
}

// Delete User
const isDeletingUser = ref(false)

function promptDeleteUser(op: OperatorItem) {
  if (isProtectedUser(op)) return
  userToDelete.value = op
  showDeleteModal.value = true
}

async function confirmDeleteUser() {
  if (userToDelete.value) {
    const id = userToDelete.value.id
    isDeletingUser.value = true
    try {
      await usersApi.delete(id)
      operators.value = operators.value.filter((op) => op.id !== id)
      selectedUserIds.value = selectedUserIds.value.filter((uid) => uid !== id)
      toast.success(t('users.deleteSuccess') || t('common.changesApplied'))
      showDeleteModal.value = false
      userToDelete.value = null
    } catch (err: any) {
      console.error('Backend delete failed:', err)
      toast.error(err?.message || t('users.deleteError') || 'Delete failed')
    } finally {
      isDeletingUser.value = false
    }
  }
}

// Toggle Lock
const isTogglingLock = ref(false)

function handleToggleLock(op: OperatorItem) {
  if (isProtectedUser(op)) return
  selectedUserForAction.value = op
  showLockModal.value = true
}

async function confirmToggleLock() {
  if (selectedUserForAction.value) {
    const newActiveState = !selectedUserForAction.value.is_active
    isTogglingLock.value = true
    try {
      await usersApi.update(selectedUserForAction.value.id, {
        is_active: newActiveState
      })
      selectedUserForAction.value.is_active = newActiveState
      if (!newActiveState) {
        selectedUserForAction.value.is_online = false
      }
      toast.success(newActiveState ? (t('users.unlockedSuccess') || t('common.active')) : (t('users.lockedSuccess') || t('common.disabled')))
      showLockModal.value = false
    } catch (err: any) {
      console.error('Backend lock toggle failed:', err)
      toast.error(err?.message || 'Lock toggle failed')
    } finally {
      isTogglingLock.value = false
    }
  }
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
  toast.success(t('common.changesApplied'))
}

function handleBulkExport(format: 'csv' | 'json') {
  const selectedList = operators.value.filter((op) => selectedUserIds.value.includes(op.id))
  if (format === 'csv') handleExportCsv(selectedList)
  else handleExportJson(selectedList)
}

const isBulkDeleting = ref(false)

function promptBulkDelete() {
  showBulkDeleteModal.value = true
}

async function confirmBulkDelete() {
  const idsToDelete = [...selectedUserIds.value]
  const successfullyDeleted: string[] = []
  isBulkDeleting.value = true
  try {
    for (const id of idsToDelete) {
      const op = operators.value.find((u) => u.id === id)
      if (op && !isProtectedUser(op)) {
        try {
          await usersApi.delete(id)
          successfullyDeleted.push(id)
        } catch (err) {
          console.warn(`Failed to delete user ${id}:`, err)
        }
      }
    }
    operators.value = operators.value.filter((op) => !successfullyDeleted.includes(op.id))
    selectedUserIds.value = selectedUserIds.value.filter((id) => !successfullyDeleted.includes(id))
    toast.success(t('users.deleteBulkSuccess', { count: successfullyDeleted.length }) || t('common.changesApplied'))
    showBulkDeleteModal.value = false
  } catch (err: any) {
    toast.error(err?.message || 'Bulk delete failed')
  } finally {
    isBulkDeleting.value = false
  }
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
  const userLevel = authStore.currentUserRoleLevel

  if (userLevel >= 4) {
    opts.push({
      value: 'superuser',
      label: superuserCount.value >= 4
        ? `${t('accessIdentity.superuser')} (${t('users.limit4')})`
        : t('accessIdentity.superuser'),
      disabled: superuserCount.value >= 4
    })
  }
  if (userLevel >= 3) {
    opts.push({ value: 'admin', label: t('accessIdentity.administrator') })
  }
  if (userLevel >= 2) {
    opts.push({ value: 'operator', label: t('accessIdentity.operator') })
  }
  opts.push({ value: 'viewer', label: t('accessIdentity.viewer') })
  return opts
})

const editRoleOptions = computed(() => {
  const opts = []
  const userLevel = authStore.currentUserRoleLevel
  const isTargetSuper = editUserForm.value.role === 'superuser'

  if (userLevel >= 4) {
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
          ? `${t('accessIdentity.superuser')} (${t('users.limit4')})`
          : t('accessIdentity.superuser'),
        disabled: superuserCount.value >= 4
      })
    }
  }
  if (userLevel >= 3) {
    opts.push({ value: 'admin', label: t('accessIdentity.administrator') })
  }
  if (userLevel >= 2) {
    opts.push({ value: 'operator', label: t('accessIdentity.operator') })
  }
  opts.push({ value: 'viewer', label: t('accessIdentity.viewer') })
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
              v-if="authStore.canManageUsers"
              variant="primary"
              size="sm"
              icon="person_add"
              @click="openAddModal"
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
                  <th v-if="authStore.canManageUsers" class="py-3 px-md w-12 text-center">
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
                  <th class="py-3 px-md">
                    <span>{{ t('users.mfaCol') }}</span>
                  </th>
                  <th v-if="authStore.canManageUsers" class="py-3 px-md text-right">
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
                  <td v-if="authStore.canManageUsers" class="py-3 px-md text-center">
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
                          class="w-10 h-10 rounded-xl flex items-center justify-center font-bold font-mono border overflow-hidden transition-all shadow-sm"
                          :class="op.role === 'superuser'
                            ? 'bg-tertiary-fixed-dim/20 border-tertiary-fixed-dim/50 text-tertiary-fixed-dim shadow-[0_0_12px_rgba(115,212,232,0.2)]'
                            : op.role === 'admin'
                            ? 'bg-primary-fixed-dim/20 border-primary-fixed-dim/50 text-primary-fixed-dim shadow-[0_0_12px_rgba(115,212,232,0.2)]'
                            : op.role === 'operator'
                            ? 'bg-cyan-500/20 border-cyan-500/50 text-cyan-300 shadow-[0_0_10px_rgba(6,182,212,0.15)]'
                            : 'bg-surface-variant/60 border-outline-variant text-on-surface'"
                        >
                          <img
                            v-if="getOperatorAvatar(op)"
                            :src="getOperatorAvatar(op)!"
                            :alt="op.full_name"
                            class="w-full h-full object-cover"
                          />
                          <span v-else class="text-xs font-bold">{{ op.initials }}</span>
                        </div>
                        <span
                          class="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-surface-container-low"
                          :class="op.is_online ? 'bg-emerald-500 shadow-[0_0_6px_#10b981]' : 'bg-outline-variant'"
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

                  <!-- 2FA / MFA Status Badge -->
                  <td class="py-3 px-md">
                    <div
                      class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-[11px] font-medium"
                      :class="getMfaBadgeInfo(op).colorClass"
                    >
                      <span class="material-symbols-outlined text-[13px]">{{ getMfaBadgeInfo(op).icon }}</span>
                      <span>{{ getMfaBadgeInfo(op).label }}</span>
                    </div>
                  </td>

                  <!-- Action Buttons -->
                  <td v-if="authStore.canManageUsers" class="py-3 px-md text-right">
                    <div class="flex items-center justify-end gap-1.5 text-on-surface-variant">
                      <!-- Edit User Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="canEditUser(op)
                          ? 'hover:text-primary-fixed-dim hover:bg-surface-variant/50 cursor-pointer text-on-surface-variant'
                          : 'opacity-30 cursor-not-allowed text-on-surface-variant'"
                        :title="canEditUser(op) ? t('users.editUser') : t('users.noPermission')"
                        :disabled="!canEditUser(op)"
                        @click="handleOpenEdit(op)"
                      >
                        <span class="material-symbols-outlined text-base">edit</span>
                      </button>

                      <!-- Lock / Unlock Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="canLockUser(op)
                          ? 'hover:text-amber-600 dark:hover:text-amber-400 hover:bg-amber-400/10 cursor-pointer text-on-surface-variant'
                          : 'opacity-30 cursor-not-allowed text-on-surface-variant'"
                        :title="canLockUser(op) ? t('users.lockUser') : isProtectedUser(op) ? t('users.protectedRoot') : t('users.noPermission')"
                        :disabled="!canLockUser(op)"
                        @click="handleToggleLock(op)"
                      >
                        <span class="material-symbols-outlined text-base">{{ op.is_active ? 'lock' : 'lock_open' }}</span>
                      </button>

                      <!-- Delete User Button -->
                      <button
                        type="button"
                        class="h-8 w-8 rounded-lg transition-colors flex items-center justify-center active:scale-95"
                        :class="canDeleteUser(op)
                          ? 'hover:text-error hover:bg-error-container/20 cursor-pointer text-on-surface-variant'
                          : 'opacity-30 cursor-not-allowed text-on-surface-variant'"
                        :title="canDeleteUser(op) ? t('users.deleteUser') : isProtectedUser(op) ? t('users.protectedRoot') : t('users.noPermission')"
                        :disabled="!canDeleteUser(op)"
                        @click="promptDeleteUser(op)"
                      >
                        <span class="material-symbols-outlined text-base">delete</span>
                      </button>
                    </div>
                  </td>
                </tr>

                <tr v-if="loading && filteredOperators.length === 0">
                  <td class="py-xl px-md text-center text-sm text-on-surface-variant" :colspan="authStore.canManageUsers ? 7 : 5">
                    <div class="flex flex-col items-center justify-center gap-2 py-6">
                      <span class="material-symbols-outlined text-3xl text-primary-fixed-dim animate-spin">progress_activity</span>
                      <p>{{ t('common.loading') }}</p>
                    </div>
                  </td>
                </tr>

                <tr v-else-if="filteredOperators.length === 0">
                  <td class="py-xl px-md text-center text-sm text-on-surface-variant" :colspan="authStore.canManageUsers ? 7 : 5">
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
        <!-- Error Alert -->
        <div v-if="formError" class="p-2.5 rounded-lg bg-error-container/40 border border-error/50 text-error text-xs flex items-center gap-2">
          <span class="material-symbols-outlined text-[16px] shrink-0">error</span>
          <span>{{ formError }}</span>
        </div>

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
          placeholder="a.morgan@aethercore.local"
          type="email"
          size="sm"
        />

        <BaseInput
          v-model="newUserForm.department"
          :label="t('users.department')"
          placeholder="e.g. Network Operations"
          size="sm"
        />

        <div class="flex flex-col gap-1">
          <BaseInput
            v-model="newUserForm.password"
            :label="t('users.password')"
            placeholder="Operator123!"
            type="password"
            size="sm"
          />
          <span class="text-[11px] text-on-surface-variant/80 leading-tight">
            {{ passwordHintText }}
          </span>
        </div>

        <BaseSelect
          v-model="newUserForm.role"
          :label="t('users.role')"
          :options="createRoleOptions"
          size="sm"
        />

        <BaseSelect
          v-model="newUserForm.force_2fa"
          :label="t('users.mfaEnforcement')"
          :options="mfaEnforceOptions"
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
        <!-- Error Alert -->
        <div v-if="editFormError" class="p-2.5 rounded-lg bg-error-container/40 border border-error/50 text-error text-xs flex items-center gap-2">
          <span class="material-symbols-outlined text-[16px] shrink-0">error</span>
          <span>{{ editFormError }}</span>
        </div>

        <!-- User Info Header -->
        <div class="p-3 bg-surface-container-high rounded-lg border border-outline-variant/50 flex items-center justify-between">
          <div class="flex flex-col">
            <span class="text-xs font-bold text-on-surface font-mono">@{{ editUserForm.username }}</span>
            <span class="text-xs text-on-surface-variant">{{ editUserForm.full_name || editUserForm.username }}</span>
          </div>
          <span class="px-2 py-0.5 rounded text-[11px] font-mono uppercase font-bold bg-surface-variant text-primary-fixed-dim">
            {{ editUserForm.role }}
          </span>
        </div>

        <!-- Role Select -->
        <BaseSelect
          v-model="editUserForm.role"
          :label="t('users.role')"
          :options="editRoleOptions"
          :disabled="editUserForm.username === 'root' || (editUserForm.role === 'superuser' && superuserCount <= 1)"
          size="sm"
        />

        <!-- 2FA Override Select -->
        <BaseSelect
          v-model="editUserForm.force_2fa"
          :label="t('users.mfaEnforcement')"
          :options="mfaEnforceOptions"
          size="sm"
        />

        <!-- Password Reset Section -->
        <div class="border-t border-outline-variant/40 pt-3 flex flex-col gap-2">
          <span class="text-xs font-semibold text-on-surface flex items-center gap-1">
            <span class="material-symbols-outlined text-[16px] text-primary-fixed-dim">lock_reset</span>
            {{ t('users.resetPassword') }}
          </span>
          <div class="flex flex-col gap-1">
            <BaseInput
              v-model="editUserForm.password"
              :label="t('users.newPasswordOptional')"
              :placeholder="t('users.passwordResetPlaceholder')"
              type="password"
              size="sm"
            />
            <span class="text-[11px] text-on-surface-variant/80 leading-tight">
              {{ editPasswordHintText }}
            </span>
          </div>

          <label v-if="editUserForm.password" class="flex items-center gap-2 pt-1 cursor-pointer select-none">
            <input
              v-model="editUserForm.must_change_password"
              type="checkbox"
              class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
            />
            <span class="text-xs text-on-surface">
              {{ t('accessIdentity.mandatoryPasswordChange') }}
            </span>
          </label>
        </div>
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
        >
          {{ isSubmitting ? t('users.saving') : t('users.save') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Modal: Lock / Unlock User -->
    <ConfirmModal
      v-model="showLockModal"
      :title="selectedUserForAction?.is_active ? t('users.lockUserModalTitle') : t('users.unlockUserModalTitle')"
      :variant="selectedUserForAction?.is_active ? 'danger' : 'primary'"
      :confirm-text="selectedUserForAction?.is_active ? t('users.lockUserAction') : t('users.unlockUserAction')"
      :cancel-text="t('users.cancel')"
      :loading="isTogglingLock"
      @confirm="confirmToggleLock"
    >
      <p v-if="selectedUserForAction">
        {{ selectedUserForAction.is_active ? t('users.lockConfirmMessage', { name: selectedUserForAction.full_name, username: selectedUserForAction.username }) : t('users.unlockConfirmMessage', { name: selectedUserForAction.full_name, username: selectedUserForAction.username }) }}
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
      :loading="isDeletingUser"
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
      :loading="isBulkDeleting"
      @confirm="confirmBulkDelete"
    >
      <p>
        {{ t('users.deleteBulkConfirmMessage', { count: selectedUserIds.length }) }}
      </p>
    </ConfirmModal>
  </div>
</template>
