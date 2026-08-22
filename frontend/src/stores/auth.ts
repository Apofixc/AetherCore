import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, type User, type AuthConfig } from '@/api/auth'
import { api } from '@/api/client'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const token = ref<string | null>(
    localStorage.getItem('aether_token') || sessionStorage.getItem('aether_token')
  )
  const authConfig = ref<AuthConfig | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const sessionExpired = ref(false)

  const isAuthRequired = computed(() => (authConfig.value ? authConfig.value.web_ui_auth : true))
  const isAuthenticated = computed(() => !isAuthRequired.value || !!token.value)
  const isSuperuser = computed(() => !isAuthRequired.value || (user.value?.is_superuser ?? false))

  const currentUserRoleLevel = computed(() => {
    if (!isAuthRequired.value || isSuperuser.value || user.value?.roles?.includes('superuser')) return 4
    if (user.value?.roles?.includes('admin')) return 3
    if (user.value?.roles?.includes('operator')) return 2
    return 1
  })

  const canManageUsers = computed(() => {
    return !isAuthRequired.value || isSuperuser.value || (user.value?.permissions?.includes('users.manage') ?? false) || (user.value?.roles?.includes('admin') ?? false)
  })

  const canManageSecurity = computed(() => {
    return !isAuthRequired.value || isSuperuser.value || (user.value?.permissions?.includes('settings.security.manage') ?? false) || (user.value?.roles?.includes('admin') ?? false)
  })

  const canManageRoles = computed(() => {
    return !isAuthRequired.value || isSuperuser.value || (user.value?.permissions?.includes('access.roles.manage') ?? false) || (user.value?.roles?.includes('admin') ?? false)
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
          roles: ['admin'],
          permissions: ['*']
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

  async function login(operatorId: string, accessCode: string, rememberMe: boolean = true) {
    loading.value = true
    error.value = null
    sessionExpired.value = false
    try {
      const response = await authApi.login(operatorId, accessCode)
      token.value = response.token
      user.value = response.user
      api.setToken(response.token)

      if (rememberMe) {
        localStorage.setItem('aether_token', response.token)
        localStorage.setItem('aether_remembered_operator', operatorId)
        sessionStorage.removeItem('aether_token')
      } else {
        sessionStorage.setItem('aether_token', response.token)
        localStorage.removeItem('aether_token')
      }

      if (authConfig.value?.inactivity_timeout) {
        startInactivityTracker()
      }

      return true
    } catch (err: any) {
      error.value = err.message || 'Authentication failed'
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
    user.value = null
    localStorage.removeItem('aether_token')
    sessionStorage.removeItem('aether_token')
    api.setToken(null)
  }

  return {
    user,
    token,
    authConfig,
    loading,
    error,
    sessionExpired,
    isAuthRequired,
    isAuthenticated,
    isSuperuser,
    currentUserRoleLevel,
    canManageUsers,
    canManageSecurity,
    canManageRoles,
    checkAuthConfig,
    login,
    fetchUser,
    logout,
    startInactivityTracker,
    stopInactivityTracker
  }
})
