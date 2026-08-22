import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, type User, type AuthConfig } from '@/api/auth'
import { api } from '@/api/client'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const token = ref<string | null>(localStorage.getItem('nms_token'))
  const authConfig = ref<AuthConfig | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const isAuthRequired = computed(() => (authConfig.value ? authConfig.value.web_ui_auth : true))
  const isAuthenticated = computed(() => !isAuthRequired.value || !!token.value)
  const isSuperuser = computed(() => !isAuthRequired.value || (user.value?.is_superuser ?? false))

  async function checkAuthConfig(): Promise<AuthConfig | null> {
    try {
      const cfg = await authApi.getConfig()
      authConfig.value = cfg
      if (!cfg.web_ui_auth && !user.value) {
        user.value = {
          id: '07611e2c-97b8-496c-91c5-30af70cba860',
          username: 'admin',
          full_name: 'System Administrator',
          email: 'admin@nms.local',
          is_active: true,
          is_superuser: true,
          must_change_password: false,
          roles: ['admin'],
          permissions: ['*']
        }
      }
      return cfg
    } catch (e) {
      console.debug('Failed to load auth config:', e)
      return null
    }
  }

  async function login(operatorId: string, accessCode: string) {
    loading.value = true
    error.value = null
    try {
      const response = await authApi.login(operatorId, accessCode)
      token.value = response.token
      user.value = response.user
      api.setToken(response.token)
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
    token.value = null
    user.value = null
    api.setToken(null)
  }

  return {
    user,
    token,
    authConfig,
    loading,
    error,
    isAuthRequired,
    isAuthenticated,
    isSuperuser,
    checkAuthConfig,
    login,
    fetchUser,
    logout
  }
})
