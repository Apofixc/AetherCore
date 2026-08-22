import { api } from './client'

export interface ModuleSubscription {
  id: string
  name_key: string
  code: string
  desc_key: string
  enabled: boolean
  mute: 'none' | '15m' | '1h' | '8h' | 'inf' | string
  sound: string
  threshold: string
}

export interface UserPreferences {
  timezone: string
  time_format?: string
  theme: 'dark' | 'light' | 'system' | string
  locale: 'ru' | 'en' | string
  department?: string
  active_mute_duration: 'none' | '15m' | '1h' | '8h' | '24h' | 'inf' | string
  quiet_hours_enabled: boolean
  quiet_schedule: string
  sound_info: string
  sound_success: string
  sound_warning: string
  sound_error: string
  module_subscriptions: ModuleSubscription[]
  sidebar_collapsed: boolean
  avatar?: string
}

export interface SecurityPolicies {
  web_ui_auth: boolean
  mandatory_password_change: boolean
  force_2fa: boolean
  mfa_scope?: 'disabled' | 'admins_only' | 'all'
  mfa_remember_device_days?: number
  mfa_grace_period_days?: number
  mfa_backup_codes_count?: number
  max_login_attempts: number
  lockout_duration: number
  session_ttl: number
  inactivity_timeout: number
  min_password_length: number
  require_uppercase: boolean
  require_digits: boolean
  require_special: boolean
  ip_whitelist: string
}

export interface PermissionItem {
  id: string
  name: string
  code: string
  description: string
  admin: boolean
  operator: boolean
  viewer: boolean
}

export interface PermissionCategory {
  id: string
  name: string
  icon: string
  items: PermissionItem[]
}

export interface MaintenanceSettings {
  auto_backup: boolean
  backup_interval_hours: number
  backup_retention_days: number
  audit_retention_days: number
  default_log_level: string
}

export const settingsApi = {
  // Персональные предпочтения пользователя
  getUserPreferences: async (): Promise<UserPreferences> => {
    return api.get<UserPreferences>('/api/v1/settings/user-preferences')
  },
  updateUserPreferences: async (prefs: Partial<UserPreferences>): Promise<UserPreferences> => {
    return api.put<UserPreferences>('/api/v1/settings/user-preferences', prefs)
  },

  // Общесистемные политики безопасности
  getSecurityPolicies: async (): Promise<SecurityPolicies> => {
    return api.get<SecurityPolicies>('/api/v1/settings/security')
  },
  updateSecurityPolicies: async (policies: Partial<SecurityPolicies>): Promise<SecurityPolicies> => {
    return api.put<SecurityPolicies>('/api/v1/settings/security', policies)
  },

  // Матрица прав доступа RBAC
  getPermissionsMatrix: async (): Promise<PermissionCategory[]> => {
    return api.get<PermissionCategory[]>('/api/v1/settings/permissions')
  },
  updatePermissionsMatrix: async (matrix: PermissionCategory[]): Promise<PermissionCategory[]> => {
    return api.put<PermissionCategory[]>('/api/v1/settings/permissions', matrix)
  },

  // Системное обслуживание
  getMaintenanceSettings: async (): Promise<MaintenanceSettings> => {
    return api.get<MaintenanceSettings>('/api/v1/settings/maintenance')
  },
  updateMaintenanceSettings: async (settings: Partial<MaintenanceSettings>): Promise<MaintenanceSettings> => {
    return api.put<MaintenanceSettings>('/api/v1/settings/maintenance', settings)
  }
}
