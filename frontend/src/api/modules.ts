import { api } from './client'

export interface ModuleManifest {
  id: string
  name: string
  version: string
  description?: string
  author?: string
  entrypoint?: string
  ui_entrypoint?: string
  permissions?: string[]
  config_schema?: Record<string, any>
  ui?: {
    type?: string
    title?: string
    icon?: string
    category?: string
  }
}

export interface ModuleDto {
  id: string
  name: string
  version: string
  is_active: boolean
  manifest: ModuleManifest
  created_at?: string
  updated_at?: string
}

export const modulesApi = {
  list: async (): Promise<ModuleDto[]> => {
    return api.get<ModuleDto[]>('/api/v1/modules')
  },
  get: async (id: string): Promise<ModuleDto> => {
    return api.get<ModuleDto>(`/api/v1/modules/${id}`)
  },
  enable: async (id: string): Promise<void> => {
    return api.post(`/api/v1/modules/${id}/enable`)
  },
  disable: async (id: string): Promise<void> => {
    return api.post(`/api/v1/modules/${id}/disable`)
  },
  getConfig: async (id: string): Promise<Record<string, any>> => {
    return api.get(`/api/v1/modules/${id}/config`)
  },
  saveConfig: async (id: string, config: Record<string, any>): Promise<void> => {
    return api.put(`/api/v1/modules/${id}/config`, config)
  }
}
