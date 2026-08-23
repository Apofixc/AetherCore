import { api } from './client'

export interface SystemInfo {
  name?: string
  version: string
  uptime_seconds: number
  dev_mode?: boolean
  safe_mode?: boolean
  is_dev?: boolean
  is_safe_mode?: boolean
}

export interface AuditLogEntry {
  id: string | number
  timestamp: string
  user_id?: string
  username?: string
  action: string
  target?: string
  status?: string
  ip_address?: string
  details?: string
}

export interface LogProvider {
  id: string
  name: string
  kind: string
  description?: string
}

export interface LogQueryResult {
  lines: string[]
  total_lines: number
  provider: string
}

export interface LogQueryParams {
  provider?: string
  limit?: number
  level?: string
  search?: string
}

export interface AuditQueryParams {
  limit?: number
  offset?: number
  search?: string
  after_id?: number
}

export const systemApi = {
  getInfo: async (): Promise<SystemInfo> => {
    return api.get<SystemInfo>('/api/v1/system/info')
  },
  getAuditLogs: async (params?: AuditQueryParams): Promise<AuditLogEntry[]> => {
    const query = new URLSearchParams()
    if (params?.limit) query.set('limit', params.limit.toString())
    if (params?.offset !== undefined) query.set('offset', params.offset.toString())
    if (params?.search) query.set('search', params.search)
    if (params?.after_id) query.set('after_id', params.after_id.toString())
    const qStr = query.toString()
    return api.get<AuditLogEntry[]>(`/api/v1/system/audit${qStr ? `?${qStr}` : ''}`)
  },
  getAuditLogsCount: async (search?: string): Promise<number> => {
    const query = new URLSearchParams()
    if (search) query.set('search', search)
    const qStr = query.toString()
    const res = await api.get<{ total: number }>(`/api/v1/system/audit/count${qStr ? `?${qStr}` : ''}`)
    return res.total ?? 0
  },
  clearAuditLogs: async (): Promise<{ success: boolean; deleted_count: number }> => {
    return api.delete<{ success: boolean; deleted_count: number }>('/api/v1/system/audit')
  },
  getProviders: async (): Promise<LogProvider[]> => {
    return api.get<LogProvider[]>('/api/v1/system/logs/providers')
  },
  getLogs: async (params?: LogQueryParams): Promise<LogQueryResult> => {
    const query = new URLSearchParams()
    if (params?.provider) query.set('provider', params.provider)
    if (params?.limit) query.set('limit', params.limit.toString())
    if (params?.level) query.set('level', params.level)
    if (params?.search) query.set('search', params.search)
    const qStr = query.toString()
    return api.get<LogQueryResult>(`/api/v1/system/logs${qStr ? `?${qStr}` : ''}`)
  },
  downloadLog: async (provider = 'system'): Promise<Blob> => {
    const token = localStorage.getItem('aether_token')
    const headers: HeadersInit = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    const res = await fetch(`/api/v1/system/logs/download?provider=${encodeURIComponent(provider)}`, {
      headers
    })
    if (!res.ok) {
      throw new Error(`Failed to download logs: HTTP ${res.status}`)
    }
    return res.blob()
  }
}
