import { api } from './client'

export interface SystemInfo {
  version: string
  uptime_seconds: number
  is_dev: boolean
  is_safe_mode: boolean
}

export interface AuditLogEntry {
  id: string
  timestamp: string
  user_id: string
  username: string
  action: string
  details?: string
}

export const systemApi = {
  getInfo: async (): Promise<SystemInfo> => {
    return api.get<SystemInfo>('/api/v1/system/info')
  },
  getAuditLogs: async (): Promise<AuditLogEntry[]> => {
    return api.get<AuditLogEntry[]>('/api/v1/system/audit')
  }
}
