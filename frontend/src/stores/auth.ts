import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, type User } from '@/api/auth'
import { api } from '@/api/client'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const token = ref<string | null>(localStorage.getItem('nms_token'))
  const loading = ref(false)
  const error = ref<string | null>(null)

  const isAuthenticated = computed(() => !!token.value)
  const isSuperuser = computed(() => user.value?.is_superuser ?? false)

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
    if (!token.value) return null
    loading.value = true
    try {
      const u = await authApi.getMe()
      user.value = u
      return u
    } catch (err) {
      logout()
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
    loading,
    error,
    isAuthenticated,
    isSuperuser,
    login,
    fetchUser,
    logout
  }
})
