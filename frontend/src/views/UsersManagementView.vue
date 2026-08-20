<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
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
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">
        <!-- Top Page Header with Title -->
        <div class="flex items-center justify-between flex-wrap gap-md">
          <div>
            <h1 class="font-display-lg text-display-lg text-on-surface font-bold">
              {{ t('users.title') }}
            </h1>
            <p class="text-sm text-on-surface-variant mt-1">
              {{ t('users.subtitle') }}
            </p>
          </div>
        </div>

        <!-- Action Bar: Search, Filters, Export, Add User -->
        <div class="flex items-center justify-between flex-wrap gap-md bg-surface-container-low p-md rounded-lg border border-outline-variant shadow-card-dark">
          <div class="flex items-center gap-md flex-wrap">
            <!-- Search Input -->
            <div class="relative flex items-center">
              <span class="material-symbols-outlined absolute left-2.5 text-base text-on-surface-variant pointer-events-none">search</span>
              <input
                v-model="searchQuery"
                type="text"
                class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-8 pr-3 text-xs font-body-mono text-on-surface w-64 focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/60"
                :placeholder="t('users.filterOperators')"
              />
            </div>

            <!-- Role / Status Select -->
            <div class="relative flex items-center">
              <select
                v-model="statusFilter"
                class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 text-xs font-bold text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer appearance-none"
              >
                <option value="all">{{ t('users.allStatuses') }}</option>
                <option value="online">{{ t('users.onlineOnly') }}</option>
                <option value="offline">{{ t('users.offlineOnly') }}</option>
                <option value="superuser">{{ t('users.superusers') }}</option>
                <option value="admin">{{ t('users.administrators') }}</option>
                <option value="operator">{{ t('users.operators') }}</option>
                <option value="viewer">{{ t('users.viewers') }}</option>
              </select>
              <span class="material-symbols-outlined text-sm text-on-surface-variant absolute right-2.5 pointer-events-none">expand_more</span>
            </div>

            <!-- Active Operators Counter -->
            <span class="text-xs text-on-surface-variant font-body-mono hidden xl:inline-block">
              {{ t('users.showingActiveOperators', { count: activeCount }) }}
            </span>
          </div>

          <!-- Actions: Export CSV, Export JSON, Add User -->
          <div class="flex items-center gap-2 flex-wrap">
            <button
              type="button"
              class="h-8 px-3 bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 cursor-pointer"
              @click="handleExportCsv"
            >
              <span class="material-symbols-outlined text-[16px]">csv</span>
              <span>{{ t('users.exportCsv') }}</span>
            </button>
            <button
              type="button"
              class="h-8 px-3 bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 cursor-pointer"
              @click="handleExportJson"
            >
              <span class="material-symbols-outlined text-[16px]">javascript</span>
              <span>{{ t('users.exportJson') }}</span>
            </button>
            <button
              type="button"
              class="h-8 px-3.5 bg-primary-fixed-dim hover:bg-primary-fixed-dim/90 text-on-primary-fixed border border-primary-fixed-dim rounded-lg text-xs font-bold uppercase flex items-center gap-1.5 active:scale-95 transition-all duration-200 shadow-glow-primary-sm hover:shadow-glow-primary-md cursor-pointer"
              @click="showAddModal = true"
            >
              <span class="material-symbols-outlined text-[18px]">person_add</span>
              <span>{{ t('users.addNewUser') }}</span>
            </button>
          </div>
        </div>

        <!-- Users Table Card -->
        <div class="bg-surface-container-low border border-outline-variant rounded-lg overflow-hidden shadow-card-dark">
          <div class="overflow-x-auto">
            <table class="w-full text-left border-collapse">
              <thead class="bg-surface-container-highest/60 text-[10px] text-on-surface-variant uppercase font-bold tracking-wider border-b border-outline-variant">
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
                        class="w-10 h-10 rounded-lg flex items-center justify-center font-bold font-body-mono shrink-0 border"
                        :class="op.role === 'superuser'
                          ? 'bg-tertiary-fixed-dim/20 border-tertiary-fixed-dim/40 text-tertiary-fixed-dim'
                          : op.role === 'admin'
                          ? 'bg-primary-fixed-dim/20 border-primary-fixed-dim/40 text-primary-fixed-dim'
                          : 'bg-surface-variant border-outline-variant text-on-surface'"
                      >
                        {{ op.initials }}
                      </div>
                      <div>
                        <p class="font-title-sm text-on-surface font-bold text-sm">{{ op.full_name }}</p>
                        <p class="text-[11px] text-on-surface-variant font-body-mono">{{ op.email }}</p>
                      </div>
                    </div>
                  </td>

                  <!-- Username / UID -->
                  <td class="py-3 px-md">
                    <p class="text-sm text-on-surface font-body-mono font-semibold">{{ op.username }}</p>
                    <p class="text-[10px] font-body-mono text-on-surface-variant uppercase">{{ op.uid }}</p>
                  </td>

                  <!-- Role Badge with Icon -->
                  <td class="py-3 px-md">
                    <div class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-variant border border-outline-variant">
                      <span
                        class="material-symbols-outlined text-sm"
                        :class="op.role === 'superuser' ? 'text-tertiary-fixed-dim' : 'text-primary-fixed-dim'"
                      >
                        {{ op.role === 'superuser' ? 'verified_user' : op.role === 'admin' ? 'admin_panel_settings' : op.role === 'operator' ? 'settings_accessibility' : 'visibility' }}
                      </span>
                      <span class="text-xs font-bold text-on-surface capitalize">
                        {{ op.role === 'superuser' ? t('accessIdentity.superuser') : op.role === 'admin' ? t('accessIdentity.administrator') : op.role === 'operator' ? t('accessIdentity.operator') : t('accessIdentity.viewer') }}
                      </span>
                    </div>
                  </td>

                  <!-- Status (Online / Offline) -->
                  <td class="py-3 px-md">
                    <div class="flex items-center gap-2">
                      <div
                        class="w-2.5 h-2.5 rounded-full"
                        :class="op.is_online
                          ? 'bg-tertiary-fixed-dim shadow-glow-tertiary-sm animate-pulse'
                          : 'bg-outline-variant'"
                      ></div>
                      <span
                        class="text-xs font-semibold"
                        :class="op.is_online ? 'text-tertiary-fixed-dim font-bold' : 'text-on-surface-variant'"
                      >
                        {{ op.is_online ? t('users.online') : t('users.offline') }}
                      </span>
                    </div>
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
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-md animate-fade-in"
      @click.self="showAddModal = false"
    >
      <div class="bg-surface-container-low border border-outline-variant rounded-xl p-lg max-w-md w-full shadow-2xl flex flex-col gap-md">
        <div class="flex items-center justify-between border-b border-outline-variant/60 pb-sm">
          <div class="flex items-center gap-2 text-primary-fixed-dim">
            <span class="material-symbols-outlined text-xl">person_add</span>
            <h3 class="text-sm font-bold text-on-surface">{{ t('users.addUserModalTitle') }}</h3>
          </div>
          <button
            type="button"
            class="text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
            @click="showAddModal = false"
          >
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>

        <form @submit.prevent="handleCreateUser" class="flex flex-col gap-sm">
          <div>
            <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
              {{ t('users.fullName') }}
            </label>
            <input
              v-model="newUserForm.full_name"
              type="text"
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-1.5 text-xs text-on-surface outline-none focus:ring-1 focus:ring-primary-fixed-dim"
              placeholder="e.g. Alex Morgan"
            />
          </div>

          <div>
            <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
              {{ t('users.username') }} *
            </label>
            <input
              v-model="newUserForm.username"
              type="text"
              required
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-1.5 text-xs font-body-mono text-on-surface outline-none focus:ring-1 focus:ring-primary-fixed-dim"
              placeholder="e.g. a.morgan"
            />
          </div>

          <div>
            <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
              {{ t('users.email') }}
            </label>
            <input
              v-model="newUserForm.email"
              type="email"
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-1.5 text-xs font-body-mono text-on-surface outline-none focus:ring-1 focus:ring-primary-fixed-dim"
              placeholder="a.morgan@nms.local"
            />
          </div>

          <div>
            <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
              {{ t('users.password') }} *
            </label>
            <input
              v-model="newUserForm.password"
              type="password"
              required
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-1.5 text-xs text-on-surface outline-none focus:ring-1 focus:ring-primary-fixed-dim"
              placeholder="••••••••"
            />
          </div>

          <div>
            <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
              {{ t('users.role') }}
            </label>
            <select
              v-model="newUserForm.role"
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-1.5 text-xs font-bold text-on-surface outline-none focus:ring-1 focus:ring-primary-fixed-dim cursor-pointer"
            >
              <option value="superuser">{{ t('accessIdentity.superuser') }}</option>
              <option value="admin">{{ t('accessIdentity.administrator') }}</option>
              <option value="operator">{{ t('accessIdentity.operator') }}</option>
              <option value="viewer">{{ t('accessIdentity.viewer') }}</option>
            </select>
          </div>

          <div class="flex items-center justify-end gap-2 pt-sm border-t border-outline-variant/60 mt-sm">
            <button
              type="button"
              class="px-4 py-1.5 text-xs font-semibold rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant transition-colors cursor-pointer"
              @click="showAddModal = false"
            >
              {{ t('users.cancel') }}
            </button>
            <button
              type="submit"
              class="px-4 py-1.5 text-xs font-bold rounded-lg bg-primary-fixed-dim text-on-primary-fixed hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm transition-all cursor-pointer disabled:opacity-50"
              :disabled="isSubmitting"
            >
              {{ isSubmitting ? t('users.creating') : t('users.create') }}
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Modal: Lock / Password Management -->
    <div
      v-if="showLockModal && selectedUserForAction"
      class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-md animate-fade-in"
      @click.self="showLockModal = false"
    >
      <div class="bg-surface-container-low border border-outline-variant rounded-xl p-lg max-w-sm w-full shadow-2xl flex flex-col gap-md">
        <div class="flex items-center gap-2 text-primary-fixed-dim">
          <span class="material-symbols-outlined text-xl">{{ selectedUserForAction.is_active ? 'lock' : 'lock_open' }}</span>
          <h3 class="text-sm font-bold text-on-surface">
            {{ selectedUserForAction.is_active ? 'Блокировка пользователя' : 'Разблокировка пользователя' }}
          </h3>
        </div>
        <p class="text-xs text-on-surface leading-relaxed">
          Вы действительно хотите {{ selectedUserForAction.is_active ? 'заблокировать' : 'разблокировать' }} оператора
          <strong class="text-primary-fixed-dim">{{ selectedUserForAction.full_name }}</strong> ({{ selectedUserForAction.username }})?
        </p>
        <div class="flex items-center justify-end gap-2 pt-sm border-t border-outline-variant/60">
          <button
            type="button"
            class="px-4 py-1.5 text-xs font-semibold rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant transition-colors cursor-pointer"
            @click="showLockModal = false"
          >
            {{ t('common.cancel') }}
          </button>
          <button
            type="button"
            class="px-4 py-1.5 text-xs font-bold rounded-lg transition-all cursor-pointer"
            :class="selectedUserForAction.is_active ? 'bg-error text-on-error hover:bg-error/90' : 'bg-tertiary-fixed-dim text-on-tertiary-fixed hover:bg-tertiary-fixed-dim/90'"
            @click="confirmToggleLock"
          >
            {{ selectedUserForAction.is_active ? 'Заблокировать' : 'Разблокировать' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
