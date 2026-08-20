import { api } from './client'

export interface User {
  id: string
  username: string
  full_name: string
  email: string
  is_active: boolean
  is_superuser: boolean
  roles: string[]
  permissions: string[]
  created_at?: string
  last_login_at?: string
}

export interface LoginResponse {
  success: boolean
  token: string
  user: User
}

export const authApi = {
  login: async (username: string, password: string): Promise<LoginResponse> => {
    return api.post<LoginResponse>('/api/v1/auth/login', { username, password })
  },
  getMe: async (): Promise<User> => {
    return api.get<User>('/api/v1/auth/me')
  }
}
