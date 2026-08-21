import { api } from './client'
import type { User } from './auth'

export interface CreateUserDto {
  username: string
  password?: string
  full_name?: string
  email?: string
  is_active?: boolean
  is_superuser?: boolean
  must_change_password?: boolean
  roles?: string[]
  permissions?: string[]
}

export interface UpdateUserDto {
  full_name?: string
  email?: string
  password?: string
  is_active?: boolean
  is_superuser?: boolean
  must_change_password?: boolean
  roles?: string[]
}

export const usersApi = {
  list: async (): Promise<User[]> => {
    return api.get<User[]>('/api/v1/users')
  },
  get: async (id: string): Promise<User> => {
    return api.get<User>(`/api/v1/users/${id}`)
  },
  create: async (dto: CreateUserDto): Promise<User> => {
    return api.post<User>('/api/v1/users', dto)
  },
  update: async (id: string, dto: UpdateUserDto): Promise<User> => {
    return api.put<User>(`/api/v1/users/${id}`, dto)
  },
  delete: async (id: string): Promise<void> => {
    return api.delete(`/api/v1/users/${id}`)
  }
}
