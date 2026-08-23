import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, type User, type AuthConfig, type LoginResponse } from '@/api/auth'
import { settingsApi } from '@/api/settings'
import { api } from '@/api/client'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const avatar = ref<string | null>(null)
  const token = ref<string | null>(
    localStorage.getItem('aether_token') || sessionStorage.getItem('aether_token')
  )
  const tempToken = ref<string | null>(null)
  const requires2fa = ref<boolean>(false)
  const authConfig = ref<AuthConfig | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const sessionExpired = ref(false)

  const isAuthRequired = computed(() => (authConfig.value ? authConfig.value.web_ui_auth : true))
  const isAuthenticated = computed(() => !isAuthRequired.value || !!token.value)
  const isSuperuser = computed(() => !isAuthRequired.value || (user.value?.is_superuser ?? false))

  function hasPermission(permission: string): boolean {
    if (!isAuthRequired.value || isSuperuser.value) return true
    const perms = user.value?.permissions || []
    if (perms.includes('*') || perms.includes(permission)) return true

    // Иерархия: manage дает view
    if (permission.endsWith('.view')) {
      const domain = permission.slice(0, -5)
      if (perms.includes(`${domain}.manage`) || perms.includes('system.manage') || user.value?.roles?.includes('admin')) {
        return true
      }
    }

    // Совместимость с составными кодами
    if ((permission === 'access.roles.view' || permission === 'settings.view' || permission === 'audit.view') &&
        (perms.includes('access.view') || perms.includes('access.manage') || perms.includes('system.view') || perms.includes('system.manage') || user.value?.roles?.includes('admin'))) {
      return true
    }

    if ((permission === 'access.roles.manage' || permission === 'settings.manage' || permission === 'settings.security.manage' || permission === 'audit.export') &&
        (perms.includes('access.manage') || perms.includes('system.manage') || user.value?.roles?.includes('admin'))) {
      return true
    }

    if (user.value?.roles?.includes('admin')) return true
    return false
  }

  const currentUserRoleLevel = computed(() => {
    if (!isAuthRequired.value || isSuperuser.value || user.value?.roles?.includes('superuser')) return 4
    if (user.value?.roles?.includes('admin')) return 3
    if (user.value?.roles?.includes('operator')) return 2
    return 1
  })

  const canViewModules = computed(() => hasPermission('modules.view'))
  const canManageModules = computed(() => hasPermission('modules.manage'))

  const canViewUsers = computed(() => hasPermission('users.view'))
  const canManageUsers = computed(() => hasPermission('users.manage'))

  const canViewAccess = computed(() => hasPermission('access.view'))
  const canManageAccess = computed(() => hasPermission('access.manage'))
  const canManageSecurity = computed(() => hasPermission('access.manage'))
  const canManageRoles = computed(() => hasPermission('access.manage'))

  const canViewSystem = computed(() => hasPermission('system.view'))
  const canManageSystem = computed(() => hasPermission('system.manage'))

  const is2faRequiredForCurrentUser = computed(() => {
    if (!user.value) return false
    if (user.value.force_2fa === true) return true
    if (user.value.force_2fa === false) return false
    const scope = authConfig.value?.mfa_scope || (authConfig.value?.force_2fa ? 'all' : 'disabled')
    if (scope === 'all') return true
    if (scope === 'admins_only') {
      return user.value.is_superuser || (user.value.roles?.some((r) => r === 'admin' || r === 'superuser') ?? false)
    }
    return false
  })

  // Inactivity tracking
  let inactivityTimer: number | null = null
  const userActivityEvents = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll']

  function resetInactivityTimer() {
    if (inactivityTimer) {
      window.clearTimeout(inactivityTimer)
      inactivityTimer = null
    }

    const timeoutMinutes = authConfig.value?.inactivity_timeout
    if (!timeoutMinutes || timeoutMinutes <= 0 || !token.value) {
      return
    }

    const timeoutMs = timeoutMinutes * 60 * 1000
    inactivityTimer = window.setTimeout(() => {
      console.warn(`User inactive for ${timeoutMinutes} minutes. Logging out...`)
      sessionExpired.value = true
      logout()
      if (window.location.pathname !== '/login') {
        window.location.href = '/login?reason=inactivity'
      }
    }, timeoutMs)
  }

  function handleUserActivity() {
    resetInactivityTimer()
  }

  function startInactivityTracker() {
    stopInactivityTracker()
    userActivityEvents.forEach((event) => {
      window.addEventListener(event, handleUserActivity, { passive: true })
    })
    resetInactivityTimer()
  }

  function stopInactivityTracker() {
    if (inactivityTimer) {
      window.clearTimeout(inactivityTimer)
      inactivityTimer = null
    }
    userActivityEvents.forEach((event) => {
      window.removeEventListener(event, handleUserActivity)
    })
  }

  async function checkAuthConfig(): Promise<AuthConfig | null> {
    try {
      const cfg = await authApi.getConfig()
      authConfig.value = cfg
      if (!cfg.web_ui_auth && !user.value) {
        user.value = {
          id: '07611e2c-97b8-496c-91c5-30af70cba860',
          username: 'admin',
          full_name: 'System Administrator',
          email: 'admin@aethercore.local',
          is_active: true,
          is_superuser: true,
          must_change_password: false,
          is_totp_enabled: false,
          roles: ['admin'],
          permissions: ['*']
        }
        try {
          const prefs = await settingsApi.getUserPreferences()
          if (prefs?.avatar) {
            avatar.value = prefs.avatar
          }
        } catch (e) {
          console.debug('Failed to load user preferences in no-auth mode:', e)
        }
      }
      if (token.value && cfg.inactivity_timeout) {
        startInactivityTracker()
      }
      return cfg
    } catch (e) {
      console.debug('Failed to load auth config:', e)
      return null
    }
  }

  async function handleSuccessfulLogin(response: LoginResponse, rememberMe: boolean, operatorId: string) {
    if (!response.token || !response.user) return false

    token.value = response.token
    user.value = response.user
    requires2fa.value = false
    tempToken.value = null
    api.setToken(response.token)

    if (rememberMe) {
      localStorage.setItem('aether_token', response.token)
      localStorage.setItem('aether_remembered_operator', operatorId)
      sessionStorage.removeItem('aether_token')
    } else {
      sessionStorage.setItem('aether_token', response.token)
      localStorage.removeItem('aether_token')
    }

    try {
      const prefs = await settingsApi.getUserPreferences()
      if (prefs?.avatar) {
        avatar.value = prefs.avatar
      } else {
        avatar.value = null
      }
    } catch (e) {
      console.debug('Failed to load preferences on login:', e)
    }

    if (authConfig.value?.inactivity_timeout) {
      startInactivityTracker()
    }

    return true
  }

  async function login(
    operatorId: string,
    accessCode: string,
    rememberMe: boolean = true,
    totpCode?: string,
    isBackupCode?: boolean
  ): Promise<{ success: boolean; requires_2fa?: boolean; temp_token?: string }> {
    loading.value = true
    error.value = null
    sessionExpired.value = false
    try {
      const response = await authApi.login(operatorId, accessCode, totpCode, isBackupCode)

      if (response.requires_2fa) {
        requires2fa.value = true
        tempToken.value = response.temp_token || null
        return { success: false, requires_2fa: true, temp_token: response.temp_token }
      }

      await handleSuccessfulLogin(response, rememberMe, operatorId)
      return { success: true, requires_2fa: false }
    } catch (err: any) {
      error.value = err.message || 'Authentication failed'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function verify2faLogin(
    code: string,
    isBackupCode: boolean = false,
    rememberMe: boolean = true,
    operatorId: string = 'admin'
  ): Promise<boolean> {
    if (!tempToken.value) {
      throw new Error('No 2FA challenge session active')
    }

    loading.value = true
    error.value = null
    try {
      const response = await authApi.verify2faLogin(tempToken.value, code, isBackupCode)
      await handleSuccessfulLogin(response, rememberMe, operatorId)
      return true
    } catch (err: any) {
      error.value = err.message || '2FA verification failed'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function fetchUser() {
    if (!isAuthRequired.value) {
      if (!user.value) {
        await checkAuthConfig()
      }
      return user.value
    }
    if (!token.value) return null
    loading.value = true
    try {
      const u = await authApi.getMe()
      user.value = u
      if (!authConfig.value) {
        await checkAuthConfig()
      }
      try {
        const prefs = await settingsApi.getUserPreferences()
        if (prefs?.avatar) {
          avatar.value = prefs.avatar
        } else {
          avatar.value = null
        }
      } catch (e) {
        console.debug('Failed to load preferences on fetchUser:', e)
      }
      startInactivityTracker()
      return u
    } catch (err) {
      logout()
      if (window.location.pathname !== '/login') {
        window.location.href = '/login'
      }
      return null
    } finally {
      loading.value = false
    }
  }

  function logout() {
    stopInactivityTracker()
    token.value = null
    tempToken.value = null
    requires2fa.value = false
    user.value = null
    avatar.value = null
    localStorage.removeItem('aether_token')
    sessionStorage.removeItem('aether_token')
    api.setToken(null)
  }

  return {
    user,
    avatar,
    token,
    tempToken,
    requires2fa,
    authConfig,
    loading,
    error,
    sessionExpired,
    isAuthRequired,
    isAuthenticated,
    isSuperuser,
    currentUserRoleLevel,
    is2faRequiredForCurrentUser,
    canViewModules,
    canManageModules,
    canViewUsers,
    canManageUsers,
    canViewAccess,
    canManageAccess,
    canManageSecurity,
    canManageRoles,
    canViewSystem,
    canManageSystem,
    hasPermission,
    checkAuthConfig,
    login,
    verify2faLogin,
    fetchUser,
    logout,
    startInactivityTracker,
    stopInactivityTracker
  }
})
