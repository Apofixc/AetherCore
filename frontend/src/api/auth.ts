import { api } from './client'

export interface User {
  id: string
  username: string
  full_name: string
  email: string
  department?: string
  is_active: boolean
  is_superuser: boolean
  must_change_password?: boolean
  is_username_locked?: boolean
  is_totp_enabled?: boolean
  force_2fa?: boolean | null
  roles: string[]
  permissions: string[]
  created_at?: string
  last_login_at?: string
  login_count?: number
}

export interface LoginResponse {
  success: boolean
  token?: string
  user?: User
  requires_2fa?: boolean
  temp_token?: string
  backup_codes_left?: number
}

export interface AuthConfig {
  web_ui_auth: boolean
  force_2fa: boolean
  mfa_scope?: 'disabled' | 'admins_only' | 'all'
  mfa_remember_device_days?: number
  mfa_grace_period_days?: number
  mfa_backup_codes_count?: number
  min_password_length: number
  require_uppercase: boolean
  require_digits: boolean
  require_special: boolean
  session_ttl?: number
  inactivity_timeout?: number
  max_login_attempts?: number
  lockout_duration?: number
}

export interface TotpSetupResponse {
  secret: string
  qr_code_url: string
  otpauth_url: string
  backup_codes: string[]
}

export const authApi = {
  login: async (
    username: string,
    password: string,
    totpCode?: string,
    isBackupCode?: boolean
  ): Promise<LoginResponse> => {
    return api.post<LoginResponse>('/api/v1/auth/login', {
      username,
      password,
      totp_code: totpCode,
      is_backup_code: isBackupCode
    })
  },
  verify2faLogin: async (
    tempToken: string,
    code: string,
    isBackupCode: boolean = false
  ): Promise<LoginResponse> => {
    return api.post<LoginResponse>('/api/v1/auth/2fa/verify-login', {
      temp_token: tempToken,
      code,
      is_backup_code: isBackupCode
    })
  },
  setup2fa: async (): Promise<TotpSetupResponse> => {
    return api.post<TotpSetupResponse>('/api/v1/auth/2fa/setup', {})
  },
  enable2fa: async (
    secret: string,
    code: string,
    backupCodes: string[]
  ): Promise<{ success: boolean; message: string }> => {
    return api.post<{ success: boolean; message: string }>('/api/v1/auth/2fa/enable', {
      secret,
      code,
      backup_codes: backupCodes
    })
  },
  disable2fa: async (
    password?: string,
    code?: string
  ): Promise<{ success: boolean; message: string }> => {
    return api.post<{ success: boolean; message: string }>('/api/v1/auth/2fa/disable', {
      password,
      code
    })
  },
  regenerateBackupCodes: async (
    password?: string
  ): Promise<{ success: boolean; backup_codes: string[] }> => {
    return api.post<{ success: boolean; backup_codes: string[] }>(
      '/api/v1/auth/2fa/backup-codes/regenerate',
      { password }
    )
  },
  getMe: async (): Promise<User> => {
    return api.get<User>('/api/v1/auth/me')
  },
  getConfig: async (): Promise<AuthConfig> => {
    return api.get<AuthConfig>('/api/v1/auth/config')
  }
}
