<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  BaseSwitch,
  AppButton,
  NumberInput,
  SearchInput,
  StatusBadge
} from '@/components/common'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'
import { settingsApi } from '@/api/settings'
import { systemApi } from '@/api/system'
import { usersApi } from '@/api/users'

const { t } = useI18n()
const authStore = useAuthStore()
const router = useRouter()

// Policies state
const initialWebUiAuth = ref(true)
const webUiAuth = ref(true)
const mandatoryPasswordChange = ref(true)
const force2FA = ref(false)
const maxLoginAttempts = ref(5)
const lockoutDuration = ref(30)
const sessionTTL = ref(12)
const inactivityTimeout = ref(30)
const minPasswordLength = ref(8)
const requireUppercase = ref(true)
const requireDigits = ref(true)
const requireSpecial = ref(true)
const ipWhitelist = ref('')

const saveSuccess = ref(false)

async function loadSavedSettings() {
  try {
    const [policies, matrix] = await Promise.all([
      settingsApi.getSecurityPolicies(),
      settingsApi.getPermissionsMatrix()
    ])

    if (policies) {
      if (typeof policies.web_ui_auth === 'boolean') {
        webUiAuth.value = policies.web_ui_auth
        initialWebUiAuth.value = policies.web_ui_auth
      }
      if (typeof policies.mandatory_password_change === 'boolean') mandatoryPasswordChange.value = policies.mandatory_password_change
      if (typeof policies.force_2fa === 'boolean') force2FA.value = policies.force_2fa
      if (typeof policies.max_login_attempts === 'number') maxLoginAttempts.value = policies.max_login_attempts
      if (typeof policies.lockout_duration === 'number') lockoutDuration.value = policies.lockout_duration
      if (typeof policies.session_ttl === 'number') sessionTTL.value = policies.session_ttl
      if (typeof policies.inactivity_timeout === 'number') inactivityTimeout.value = policies.inactivity_timeout
      if (typeof policies.min_password_length === 'number') minPasswordLength.value = policies.min_password_length
      if (typeof policies.require_uppercase === 'boolean') requireUppercase.value = policies.require_uppercase
      if (typeof policies.require_digits === 'boolean') requireDigits.value = policies.require_digits
      if (typeof policies.require_special === 'boolean') requireSpecial.value = policies.require_special
      if (typeof policies.ip_whitelist === 'string') ipWhitelist.value = policies.ip_whitelist
    }

    if (matrix && Array.isArray(matrix)) {
      permissionCategories.value = matrix
    }
  } catch (err) {
    console.debug('Could not load security policies from server:', err)
  }
}

async function fetchRolesAndUsers() {
  try {
    const users = await usersApi.list()
    roleCounts.value = {
      superuser: users.filter((u) => u.is_superuser).length,
      admin: users.filter((u) => !u.is_superuser && u.roles?.includes('admin')).length,
      operator: users.filter((u) => u.roles?.includes('operator')).length,
      viewer: users.filter((u) => u.roles?.includes('viewer')).length
    }
  } catch (e) {
    console.debug('Failed to load user counts for roles', e)
  }
}

async function fetchAuditLogs() {
  try {
    const rawLogs = await systemApi.getAuditLogs({ limit: 50 })
    if (Array.isArray(rawLogs)) {
      auditLogs.value = rawLogs.map((l: any) => ({
        id: `#LOG-${l.id ? l.id.slice(0, 6) : Math.floor(Math.random() * 10000)}`,
        timestamp: l.created_at ? new Date(l.created_at).toLocaleString() : new Date().toLocaleString(),
        user: l.username || 'system',
        action: l.action || 'Action',
        actionType: (l.action && l.action.includes('login') ? 'login' : l.action && l.action.includes('role') ? 'role' : l.action && l.action.includes('policy') ? 'policy' : l.action && l.action.includes('backup') ? 'backup' : 'policy') as any,
        resource: l.resource || 'system',
        details: l.status === 'success' ? `${l.action} on ${l.resource}` : `Status: ${l.status}`,
        ip: l.ip_address || '127.0.0.1'
      }))
    }
  } catch (err) {
    console.debug('Failed to fetch audit logs:', err)
  }
}

onMounted(async () => {
  await Promise.all([
    loadSavedSettings(),
    fetchRolesAndUsers(),
    fetchAuditLogs()
  ])
})

async function applyChanges() {
  const isEnablingAuth = webUiAuth.value && (!initialWebUiAuth.value || !authStore.token)

  const policiesPayload = {
    web_ui_auth: webUiAuth.value,
    mandatory_password_change: mandatoryPasswordChange.value,
    force_2fa: force2FA.value,
    max_login_attempts: maxLoginAttempts.value,
    lockout_duration: lockoutDuration.value,
    session_ttl: sessionTTL.value,
    inactivity_timeout: inactivityTimeout.value,
    min_password_length: minPasswordLength.value,
    require_uppercase: requireUppercase.value,
    require_digits: requireDigits.value,
    require_special: requireSpecial.value,
    ip_whitelist: ipWhitelist.value
  }

  try {
    // 1. Сохраняем сначала матрицу прав, пока сессия валидна
    await settingsApi.updatePermissionsMatrix(permissionCategories.value)
    // 2. Затем сохраняем политики безопасности
    await settingsApi.updateSecurityPolicies(policiesPayload)
    await authStore.checkAuthConfig()

    if (isEnablingAuth) {
      authStore.logout()
      window.location.href = '/login'
      return
    }

    initialWebUiAuth.value = webUiAuth.value
  } catch (err) {
    console.error('Could not save security policies to server:', err)
    if (isEnablingAuth) {
      authStore.logout()
      window.location.href = '/login'
      return
    }
  }

  saveSuccess.value = true
  setTimeout(() => {
    saveSuccess.value = false
  }, 3000)
}

// Roles state
const roleCounts = ref({
  superuser: 0,
  admin: 0,
  operator: 0,
  viewer: 0
})

// Permissions Matrix
interface PermissionItem {
  id: string
  name: string
  code: string
  description: string
  admin: boolean
  operator: boolean
  viewer: boolean
}

interface PermissionCategory {
  id: string
  name: string
  icon: string
  items: PermissionItem[]
}

const permissionsSearch = ref('')
const permissionCategories = ref<PermissionCategory[]>([])

const filteredCategories = computed(() => {
  if (!permissionsSearch.value.trim()) return permissionCategories.value
  const query = permissionsSearch.value.toLowerCase()
  return permissionCategories.value
    .map((cat: PermissionCategory) => ({
      ...cat,
      items: cat.items.filter((item: PermissionItem) =>
        item.name.toLowerCase().includes(query) ||
        item.code.toLowerCase().includes(query) ||
        item.description.toLowerCase().includes(query)
      )
    }))
    .filter((cat: PermissionCategory) => cat.items.length > 0)
})

function toggleCategoryRole(catId: string, role: 'admin' | 'operator' | 'viewer') {
  const cat = permissionCategories.value.find((c: PermissionCategory) => c.id === catId)
  if (!cat) return
  const allChecked = cat.items.every((i: PermissionItem) => i[role])
  cat.items.forEach((i: PermissionItem) => {
    i[role] = !allChecked
  })
}

function isCategoryRoleAll(catId: string, role: 'admin' | 'operator' | 'viewer') {
  const cat = permissionCategories.value.find((c: PermissionCategory) => c.id === catId)
  if (!cat || cat.items.length === 0) return false
  return cat.items.every((i: PermissionItem) => i[role])
}

// Audit Log State
interface AuditLogEntry {
  id: string
  timestamp: string
  user: string
  action: string
  actionType: 'role' | 'login' | 'policy' | 'failed' | 'backup'
  resource: string
  details: string
  ip: string
}

const auditSearch = ref('')
const auditLogs = ref<AuditLogEntry[]>([])

const isRefreshingLogs = ref(false)

async function handleRefreshLogs() {
  isRefreshingLogs.value = true
  await fetchAuditLogs()
  isRefreshingLogs.value = false
}

function handleExportLogs() {
  const dataStr = 'data:text/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(auditLogs.value, null, 2))
  const downloadAnchor = document.createElement('a')
  downloadAnchor.setAttribute('href', dataStr)
  downloadAnchor.setAttribute('download', `security_audit_logs_${new Date().toISOString().slice(0, 10)}.json`)
  document.body.appendChild(downloadAnchor)
  downloadAnchor.click()
  downloadAnchor.remove()
}

const filteredAuditLogs = computed(() => {
  if (!auditSearch.value.trim()) return auditLogs.value
  const query = auditSearch.value.toLowerCase()
  return auditLogs.value.filter((log: AuditLogEntry) =>
    log.id.toLowerCase().includes(query) ||
    log.user.toLowerCase().includes(query) ||
    log.action.toLowerCase().includes(query) ||
    log.resource.toLowerCase().includes(query) ||
    log.details.toLowerCase().includes(query) ||
    log.ip.toLowerCase().includes(query)
  )
})
</script>

<template>
  <div class="flex-1 flex flex-col h-full overflow-hidden select-none">
    <!-- Secondary Top Navigation Bar -->
    <SettingsNav />

    <!-- Main Content -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">

        <!-- Top Page Header -->
        <div class="flex items-center justify-between flex-wrap gap-md">
          <div class="flex items-center gap-sm text-on-surface">
            <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
              <span class="material-symbols-outlined text-xl">shield</span>
            </div>
            <div>
              <h1 class="font-display-lg text-display-lg text-on-surface font-bold">{{ t('accessIdentity.title') }}</h1>
              <p class="text-xs text-on-surface-variant mt-0.5">{{ t('accessIdentity.subtitle') }}</p>
            </div>
          </div>
          <div class="flex items-center gap-3">
            <span v-if="saveSuccess" class="text-xs text-tertiary-fixed-dim font-bold flex items-center gap-1 animate-fade-in">
              <span class="material-symbols-outlined text-[16px]">check_circle</span>
              {{ t('common.changesApplied') }}
            </span>
            <AppButton
              variant="primary"
              size="sm"
              icon="save"
              :disabled="!authStore.canManageSecurity && !authStore.canManageRoles"
              :title="(!authStore.canManageSecurity && !authStore.canManageRoles) ? t('users.noPermission') : ''"
              @click="applyChanges"
            >
              {{ t('accessIdentity.applyChanges') }}
            </AppButton>
          </div>
        </div>

        <!-- SECTION 1: Security Policies -->
        <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg flex flex-col gap-lg shadow-card-dark">
          <!-- Header -->
          <div class="flex items-center gap-sm text-on-surface">
            <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
              <span class="material-symbols-outlined text-xl">security</span>
            </div>
            <div>
              <h2 class="font-title-sm font-bold text-on-surface">{{ t('accessIdentity.securityPolicies') }}</h2>
              <p class="text-xs text-on-surface-variant mt-0.5">{{ t('accessIdentity.securityPoliciesDesc') }}</p>
            </div>
          </div>

          <!-- Row 1: 3 Global Auth Toggles -->
          <div class="grid grid-cols-1 md:grid-cols-3 gap-md">
            <BaseSwitch
              v-model="webUiAuth"
              :label="t('accessIdentity.webUiAuth')"
              :description="t('accessIdentity.webUiAuthDesc')"
              icon="login"
            />

            <BaseSwitch
              v-model="mandatoryPasswordChange"
              :label="t('accessIdentity.mandatoryPasswordChange')"
              :description="t('accessIdentity.mandatoryPasswordChangeDesc')"
              icon="lock_reset"
            />

            <BaseSwitch
              v-model="force2FA"
              :label="t('accessIdentity.force2FA')"
              :description="t('accessIdentity.force2FADesc')"
              icon="phonelink_lock"
            />
          </div>

          <!-- Row 2: Rate Limiting & Session Lifecycle Grid -->
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-lg">
            <!-- Rate Limiting & Lockout -->
            <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-md">
              <div class="flex items-center gap-2.5">
                <span class="material-symbols-outlined text-primary-fixed-dim text-lg">lock_clock</span>
                <div>
                  <h3 class="text-xs font-bold uppercase tracking-wider text-on-surface">{{ t('accessIdentity.rateLimitingLockout') }}</h3>
                  <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('accessIdentity.rateLimitingLockoutDesc') }}</p>
                </div>
              </div>
              <div class="flex flex-col gap-sm">
                <div class="flex items-center justify-between">
                  <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.maxLoginAttempts') }}</span>
                  <NumberInput
                    v-model="maxLoginAttempts"
                    :min="1"
                    :max="20"
                    width-class="w-20"
                  />
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.lockoutDuration') }}</span>
                  <NumberInput
                    v-model="lockoutDuration"
                    :min="1"
                    :max="1440"
                    width-class="w-20"
                  />
                </div>
              </div>
            </div>

            <!-- Session Lifecycle -->
            <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-md">
              <div class="flex items-center gap-2.5">
                <span class="material-symbols-outlined text-primary-fixed-dim text-lg">schedule</span>
                <div>
                  <h3 class="text-xs font-bold uppercase tracking-wider text-on-surface">{{ t('accessIdentity.sessionLifecycle') }}</h3>
                  <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('accessIdentity.sessionLifecycleDesc') }}</p>
                </div>
              </div>
              <div class="flex flex-col gap-sm">
                <div class="flex items-center justify-between">
                  <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.sessionTTL') }}</span>
                  <NumberInput
                    v-model="sessionTTL"
                    :min="1"
                    :max="720"
                    width-class="w-20"
                  />
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.inactivityTimeout') }}</span>
                  <NumberInput
                    v-model="inactivityTimeout"
                    :min="1"
                    :max="1440"
                    width-class="w-20"
                  />
                </div>
              </div>
            </div>
          </div>

          <!-- Row 3: Password Complexity Policy -->
          <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-md">
            <div class="flex items-center gap-2.5">
              <span class="material-symbols-outlined text-primary-fixed-dim text-lg">vpn_key</span>
              <div>
                <h3 class="text-xs font-bold uppercase tracking-wider text-on-surface">{{ t('accessIdentity.passwordComplexity') }}</h3>
                <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('accessIdentity.passwordComplexityDesc') }}</p>
              </div>
            </div>
            <div class="flex flex-wrap items-center gap-xl">
              <div class="flex items-center gap-3">
                <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.minLength') }}</span>
                <NumberInput
                  v-model="minPasswordLength"
                  :min="4"
                  :max="64"
                  width-class="w-24"
                />
              </div>
              <label class="flex items-center gap-2 cursor-pointer">
                <input class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0" type="checkbox" v-model="requireUppercase">
                <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.uppercase') }}</span>
              </label>
              <label class="flex items-center gap-2 cursor-pointer">
                <input class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0" type="checkbox" v-model="requireDigits">
                <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.digits') }}</span>
              </label>
              <label class="flex items-center gap-2 cursor-pointer">
                <input class="rounded border-outline-variant bg-surface-container-lowest text-primary-fixed-dim focus:ring-0" type="checkbox" v-model="requireSpecial">
                <span class="text-xs text-on-surface-variant">{{ t('accessIdentity.specialChars') }}</span>
              </label>
            </div>
          </div>

          <!-- Row 4: Allowed IPs / Subnets (IP Whitelist) -->
          <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-sm">
            <div class="flex items-center gap-2.5">
              <span class="material-symbols-outlined text-primary-fixed-dim text-lg">lan</span>
              <div>
                <h3 class="text-xs font-bold uppercase tracking-wider text-on-surface">{{ t('accessIdentity.ipWhitelist') }}</h3>
                <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('accessIdentity.ipWhitelistDesc') }}</p>
              </div>
            </div>
            <input
              class="w-full bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-xs font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none"
              type="text"
              v-model="ipWhitelist"
            >
          </div>
        </div>

        <!-- SECTION 2: Permissions Matrix -->
        <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg flex flex-col gap-lg shadow-card-dark">
          <!-- Group Header -->
          <div class="flex items-center justify-between flex-wrap gap-md">
            <div class="flex items-center gap-sm text-on-surface">
              <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                <span class="material-symbols-outlined text-xl">admin_panel_settings</span>
              </div>
              <div>
                <h2 class="font-title-sm font-bold text-on-surface">{{ t('accessIdentity.permissionsMatrix') }}</h2>
                <p class="text-xs text-on-surface-variant mt-0.5">{{ t('accessIdentity.permissionsMatrixDesc') }}</p>
              </div>
            </div>
            <!-- Search input -->
            <SearchInput
              v-model="permissionsSearch"
              :placeholder="t('accessIdentity.searchPermissions')"
              width-class="w-64"
            />
          </div>

          <!-- Permissions Matrix Table -->
          <div class="bg-surface-container border border-outline-variant rounded-lg overflow-hidden flex flex-col">
            <div class="overflow-x-auto">
              <table class="w-full text-left border-collapse">
                <thead class="bg-surface-container-highest/60 text-[10px] text-on-surface-variant uppercase font-bold tracking-wider border-b border-outline-variant">
                  <tr>
                    <th class="py-3 px-lg min-w-[340px]">{{ t('accessIdentity.permission') }}</th>
                    <th class="py-3 px-lg text-center w-36">
                      <div class="flex items-center justify-center gap-1.5">
                        <span>{{ t('accessIdentity.superuser') }}</span>
                        <span class="px-1.5 py-0.5 rounded text-[10px] font-body-mono bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30">
                          {{ roleCounts.superuser }}
                        </span>
                      </div>
                    </th>
                    <th class="py-3 px-lg text-center w-36">
                      <div class="flex items-center justify-center gap-1.5">
                        <span>{{ t('accessIdentity.administrator') }}</span>
                        <span class="px-1.5 py-0.5 rounded text-[10px] font-body-mono bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30">
                          {{ roleCounts.admin }}
                        </span>
                      </div>
                    </th>
                    <th class="py-3 px-lg text-center w-36">
                      <div class="flex items-center justify-center gap-1.5">
                        <span>{{ t('accessIdentity.operator') }}</span>
                        <span class="px-1.5 py-0.5 rounded text-[10px] font-body-mono bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30">
                          {{ roleCounts.operator }}
                        </span>
                      </div>
                    </th>
                    <th class="py-3 px-lg text-center w-36">
                      <div class="flex items-center justify-center gap-1.5">
                        <span>{{ t('accessIdentity.viewer') }}</span>
                        <span class="px-1.5 py-0.5 rounded text-[10px] font-body-mono bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30">
                          {{ roleCounts.viewer }}
                        </span>
                      </div>
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-outline-variant/30 text-xs">
                  <template v-for="cat in filteredCategories" :key="cat.id">
                    <!-- Category Header Row -->
                    <tr class="bg-surface-container-highest/40 font-bold border-t border-outline-variant/60">
                      <td class="py-2.5 px-lg text-primary-fixed-dim flex items-center gap-2">
                        <span class="material-symbols-outlined text-base">{{ cat.icon }}</span>
                        <span>{{ cat.name }} <span class="text-xs font-normal opacity-80">({{ cat.items.length }})</span></span>
                      </td>
                      <td class="py-2.5 px-lg text-center text-[10px] text-on-surface-variant font-body-mono uppercase">{{ t('common.all') }}</td>
                      <td class="py-2.5 px-lg text-center">
                        <button
                          v-if="authStore.isSuperuser"
                          type="button"
                          class="text-[10px] text-primary-fixed-dim hover:underline font-bold cursor-pointer"
                          @click="toggleCategoryRole(cat.id, 'admin')"
                        >
                          {{ isCategoryRoleAll(cat.id, 'admin') ? t('common.clearAll') : t('common.selectAll') }}
                        </button>
                        <span v-else class="text-[10px] text-on-surface-variant font-body-mono opacity-50">{{ t('common.all') }}</span>
                      </td>
                      <td class="py-2.5 px-lg text-center">
                        <button
                          v-if="authStore.currentUserRoleLevel >= 3"
                          type="button"
                          class="text-[10px] text-primary-fixed-dim hover:underline font-bold cursor-pointer"
                          @click="toggleCategoryRole(cat.id, 'operator')"
                        >
                          {{ isCategoryRoleAll(cat.id, 'operator') ? t('common.clearAll') : t('common.selectAll') }}
                        </button>
                        <span v-else class="text-[10px] text-on-surface-variant font-body-mono opacity-50">{{ t('common.all') }}</span>
                      </td>
                      <td class="py-2.5 px-lg text-center">
                        <button
                          v-if="authStore.currentUserRoleLevel >= 3"
                          type="button"
                          class="text-[10px] text-primary-fixed-dim hover:underline font-bold cursor-pointer"
                          @click="toggleCategoryRole(cat.id, 'viewer')"
                        >
                          {{ isCategoryRoleAll(cat.id, 'viewer') ? t('common.clearAll') : t('common.selectAll') }}
                        </button>
                        <span v-else class="text-[10px] text-on-surface-variant font-body-mono opacity-50">{{ t('common.all') }}</span>
                      </td>
                    </tr>

                    <!-- Category Items Rows -->
                    <tr
                      v-for="item in cat.items"
                      :key="item.id"
                      class="hover:bg-surface-variant/20 transition-colors"
                    >
                      <td class="py-3 px-lg pl-10">
                        <div class="font-bold text-on-surface">
                          {{ item.name }}
                          <span class="font-body-mono text-[11px] text-on-surface-variant font-normal">({{ item.code }})</span>
                        </div>
                        <div class="text-[10px] text-on-surface-variant">{{ item.description }}</div>
                      </td>
                      <!-- Superuser (always checked & disabled) -->
                      <td class="py-3 px-lg text-center">
                        <div class="flex justify-center">
                          <label class="relative inline-flex items-center justify-center cursor-not-allowed opacity-80">
                            <input class="sr-only peer" type="checkbox" checked disabled>
                            <div class="w-10 h-5 bg-primary-fixed-dim rounded-full border border-primary-fixed-dim relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-primary after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5"></div>
                          </label>
                        </div>
                      </td>
                      <!-- Administrator -->
                      <td class="py-3 px-lg text-center">
                        <label class="relative inline-flex items-center justify-center" :class="authStore.isSuperuser ? 'cursor-pointer' : 'cursor-not-allowed opacity-60'">
                          <input class="sr-only peer" type="checkbox" v-model="item.admin" :disabled="!authStore.isSuperuser">
                          <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
                        </label>
                      </td>
                      <!-- Operator -->
                      <td class="py-3 px-lg text-center">
                        <label class="relative inline-flex items-center justify-center" :class="authStore.currentUserRoleLevel >= 3 ? 'cursor-pointer' : 'cursor-not-allowed opacity-60'">
                          <input class="sr-only peer" type="checkbox" v-model="item.operator" :disabled="authStore.currentUserRoleLevel < 3">
                          <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
                        </label>
                      </td>
                      <!-- Viewer -->
                      <td class="py-3 px-lg text-center">
                        <label class="relative inline-flex items-center justify-center" :class="authStore.currentUserRoleLevel >= 3 ? 'cursor-pointer' : 'cursor-not-allowed opacity-60'">
                          <input class="sr-only peer" type="checkbox" v-model="item.viewer" :disabled="authStore.currentUserRoleLevel < 3">
                          <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
                        </label>
                      </td>
                    </tr>
                  </template>
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <!-- SECTION 3: Security Audit Log -->
        <div class="bg-surface-container-low border border-outline-variant rounded-lg overflow-hidden shadow-card-dark">
          <!-- Card Header -->
          <div class="p-lg border-b border-outline-variant bg-surface-container flex items-center justify-between flex-wrap gap-md">
            <div class="flex items-center gap-sm">
              <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                <span class="material-symbols-outlined text-xl">verified_user</span>
              </div>
              <div>
                <h2 class="font-title-sm font-bold text-on-surface">{{ t('accessIdentity.securityAuditLog') }}</h2>
                <p class="text-xs text-on-surface-variant mt-0.5">{{ t('accessIdentity.securityAuditLogDesc') }}</p>
              </div>
            </div>
            <!-- Action Group: Search, Filter, Refresh, Export -->
            <div class="flex items-center gap-2 flex-wrap">
              <!-- Search Input -->
              <SearchInput
                v-model="auditSearch"
                :placeholder="t('accessIdentity.searchAuditPlaceholder')"
                width-class="w-64"
              />

              <!-- Filter Button -->
              <button
                type="button"
                class="h-8 w-8 bg-surface-container-highest border border-outline-variant rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant transition-colors cursor-pointer flex items-center justify-center shrink-0 active:scale-95"
                :title="t('common.filter')"
              >
                <span class="material-symbols-outlined text-[18px]">filter_list</span>
              </button>

              <!-- Refresh Button -->
              <button
                type="button"
                class="h-8 w-8 bg-surface-container-highest border border-outline-variant rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant transition-colors cursor-pointer flex items-center justify-center shrink-0 active:scale-95"
                :title="t('common.refresh')"
                @click="handleRefreshLogs"
              >
                <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isRefreshingLogs }">refresh</span>
              </button>

              <!-- Export Button -->
              <button
                type="button"
                class="h-8 w-8 bg-surface-container-highest border border-outline-variant rounded-lg text-on-surface-variant hover:text-primary-fixed-dim hover:bg-surface-variant transition-colors cursor-pointer flex items-center justify-center shrink-0 active:scale-95"
                :title="t('accessIdentity.exportLogs')"
                @click="handleExportLogs"
              >
                <span class="material-symbols-outlined text-[18px]">download</span>
              </button>
            </div>
          </div>

          <!-- Table -->
          <div class="overflow-x-auto">
            <table class="w-full text-left border-collapse">
              <thead class="bg-surface-container-highest/60 text-[10px] text-on-surface-variant uppercase font-bold tracking-wider border-b border-outline-variant">
                <tr>
                  <th class="py-3 px-lg">{{ t('accessIdentity.id') }}</th>
                  <th class="py-3 px-lg">{{ t('accessIdentity.timestamp') }}</th>
                  <th class="py-3 px-lg">{{ t('accessIdentity.user') }}</th>
                  <th class="py-3 px-lg">{{ t('accessIdentity.action') }}</th>
                  <th class="py-3 px-lg">{{ t('accessIdentity.resource') }}</th>
                  <th class="py-3 px-lg">{{ t('accessIdentity.details') }}</th>
                  <th class="py-3 px-lg text-right">{{ t('accessIdentity.ipAddress') }}</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-outline-variant/30 text-xs">
                <tr
                  v-for="log in filteredAuditLogs"
                  :key="log.id"
                  class="hover:bg-surface-variant/20 transition-colors"
                >
                  <td class="py-3 px-lg font-body-mono text-on-surface-variant text-[11px]">{{ log.id }}</td>
                  <td class="py-3 px-lg font-body-mono text-on-surface-variant text-[11px]">{{ log.timestamp }}</td>
                  <td class="py-3 px-lg font-bold text-on-surface font-body-mono">{{ log.user }}</td>
                  <td class="py-3 px-lg">
                    <span
                      v-if="log.actionType === 'role' || log.actionType === 'policy'"
                      class="bg-primary-fixed-dim/10 text-primary-fixed-dim border border-primary-fixed-dim/30 px-2 py-0.5 rounded text-[10px] font-bold uppercase"
                    >
                      {{ log.action }}
                    </span>
                    <span
                      v-else-if="log.actionType === 'login' || log.actionType === 'backup'"
                      class="bg-tertiary-fixed-dim/10 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30 px-2 py-0.5 rounded text-[10px] font-bold uppercase"
                    >
                      {{ log.action }}
                    </span>
                    <span
                      v-else
                      class="bg-error/10 text-error border border-error/30 px-2 py-0.5 rounded text-[10px] font-bold uppercase"
                    >
                      {{ log.action }}
                    </span>
                  </td>
                  <td class="py-3 px-lg font-body-mono text-on-surface-variant">{{ log.resource }}</td>
                  <td
                    class="py-3 px-lg"
                    :class="log.actionType === 'failed' ? 'text-error' : 'text-on-surface-variant'"
                  >
                    {{ log.details }}
                  </td>
                  <td
                    class="py-3 px-lg text-right font-body-mono"
                    :class="log.actionType === 'failed' ? 'text-error' : 'text-on-surface-variant'"
                  >
                    {{ log.ip }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- Pagination Footer -->
          <div class="p-md border-t border-outline-variant bg-surface-container flex items-center justify-between flex-wrap gap-md">
            <div class="text-xs text-on-surface-variant font-body-mono">
              {{ t('common.totalEvents', { total: filteredAuditLogs.length, count: Math.min(filteredAuditLogs.length, 5) }) }}
            </div>
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="px-3 py-1 bg-surface-container-highest border border-outline-variant rounded-lg text-xs text-on-surface-variant hover:text-on-surface disabled:opacity-40 cursor-pointer"
                disabled
              >
                {{ t('common.previous') }}
              </button>
              <span class="text-xs text-on-surface-variant font-body-mono px-2">{{ t('common.pageOf', { page: 1, total: 1 }) }}</span>
              <button
                type="button"
                class="px-3 py-1 bg-surface-container-highest border border-outline-variant rounded-lg text-xs text-on-surface hover:bg-surface-variant transition-colors cursor-pointer"
              >
                {{ t('common.next') }}
              </button>
            </div>
          </div>
        </div>

      </div>
    </main>
  </div>
</template>
