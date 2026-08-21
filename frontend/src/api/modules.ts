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
  is_enabled?: boolean
  description?: string
  manifest: ModuleManifest
  created_at?: string
  updated_at?: string
}

function normalizeModule(mod: any): ModuleDto {
  return {
    ...mod,
    is_active: mod.is_active ?? mod.is_enabled ?? true,
    is_enabled: mod.is_enabled ?? mod.is_active ?? true,
    name: mod.name || mod.manifest?.name || mod.id,
    version: mod.version || mod.manifest?.version || '1.0.0'
  }
}

export const modulesApi = {
  list: async (): Promise<ModuleDto[]> => {
    const list = await api.get<any[]>('/api/v1/modules')
    return Array.isArray(list) ? list.map(normalizeModule) : []
  },
  get: async (id: string): Promise<ModuleDto> => {
    const mod = await api.get<any>(`/api/v1/modules/${id}`)
    return normalizeModule(mod)
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
