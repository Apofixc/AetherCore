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

export interface AuditArchiveInfo {
  filename: string
  size_bytes: number
  created_at: string
  records_count?: number
}

export interface RotateAuditResponse {
  success: boolean
  deleted_count: number
  archive_filename?: string
}

export interface ImportAuditResponse {
  success: boolean
  imported_count: number
}

export interface DbStorageStats {
  db_size_bytes: number
  wal_size_bytes: number
  shm_size_bytes: number
  total_size_bytes: number
  page_size: number
  page_count: number
  freelist_count: number
  tables_count: number
  wal_mode: boolean
}

export interface BackupFileInfo {
  filename: string
  size_bytes: number
  created_at: string
  tag: string
  is_valid: boolean
}

export interface DbStatsResponse {
  storage: DbStorageStats
  latest_backup: BackupFileInfo | null
  total_backups_count: number
}

export interface RestoreResult {
  success: boolean
  pre_restore_backup?: string
  message: string
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
  rotateAuditLogs: async (params?: { days?: number; archive?: boolean }): Promise<RotateAuditResponse> => {
    return api.post<RotateAuditResponse>('/api/v1/system/audit/rotate', {
      days: params?.days ?? 90,
      archive: params?.archive ?? true
    })
  },
  importAuditLogs: async (records: any[]): Promise<ImportAuditResponse> => {
    return api.post<ImportAuditResponse>('/api/v1/system/audit/import', { records })
  },
  getAuditArchives: async (): Promise<AuditArchiveInfo[]> => {
    return api.get<AuditArchiveInfo[]>('/api/v1/system/audit/archives')
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
  },
  getDbStats: async (): Promise<DbStatsResponse> => {
    return api.get<DbStatsResponse>('/api/v1/system/db/stats')
  },
  getBackups: async (): Promise<BackupFileInfo[]> => {
    return api.get<BackupFileInfo[]>('/api/v1/system/backup/list')
  },
  createBackup: async (tag?: string): Promise<BackupFileInfo> => {
    return api.post<BackupFileInfo>('/api/v1/system/backup/create', { tag })
  },
  downloadBackup: async (filename: string): Promise<Blob> => {
    const token = localStorage.getItem('aether_token')
    const headers: HeadersInit = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    const res = await fetch(`/api/v1/system/backup/download/${encodeURIComponent(filename)}`, {
      headers
    })
    if (!res.ok) {
      throw new Error(`Failed to download backup: HTTP ${res.status}`)
    }
    return res.blob()
  },
  restoreBackup: async (filename: string): Promise<RestoreResult> => {
    return api.post<RestoreResult>('/api/v1/system/backup/restore', { filename })
  },
  uploadAndRestoreBackup: async (file: File): Promise<RestoreResult> => {
    const token = localStorage.getItem('aether_token')
    const formData = new FormData()
    formData.append('file', file)
    const headers: HeadersInit = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    const res = await fetch('/api/v1/system/backup/upload-restore', {
      method: 'POST',
      headers,
      body: formData
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({ message: `HTTP ${res.status}` }))
      throw new Error(err.message || err.error || `HTTP ${res.status}`)
    }
    return res.json()
  },
  deleteBackup: async (filename: string): Promise<{ success: boolean; deleted: string }> => {
    return api.delete<{ success: boolean; deleted: string }>(`/api/v1/system/backup/${encodeURIComponent(filename)}`)
  }
}
