import { api } from './client'

export type TaskScheduleType =
  | { type: 'cron'; value: string }
  | { type: 'interval_sec'; value: number }
  | { type: 'one_off'; value: string }

export type TaskActionType =
  | { type: 'system_audit_rotation' }
  | { type: 'system_history_cleanup' }
  | { type: 'system_db_backup' }
  | { type: 'plugin_timer'; params: { module_id: string; timer_id: string } }
  | { type: 'event_bus_publish'; params: { topic: string; payload: any } }

export type ConcurrencyPolicy = 'skip' | 'allow' | 'queue'
export type MisfirePolicy = 'skip_to_next' | 'fire_once_immediately'
export type ExecutionStatus = 'success' | 'failed' | 'timeout' | 'skipped' | 'aborted' | 'running'

export interface ScheduledTask {
  id: string
  name: string
  description?: string
  schedule: TaskScheduleType
  action: TaskActionType
  concurrency_policy: ConcurrencyPolicy
  misfire_policy: MisfirePolicy
  timeout_secs: number
  is_enabled: boolean
  is_system: boolean
  next_run_at?: string
  last_run_at?: string
  last_status?: ExecutionStatus
  last_error?: string
  created_at: string
  updated_at: string
}

export interface TaskExecutionRecord {
  id: number
  task_id: string
  task_name: string
  started_at: string
  finished_at: string
  status: ExecutionStatus
  duration_ms: number
  error_message?: string
  triggered_by: string
}

export interface CreateTaskDto {
  id?: string
  name: string
  description?: string
  schedule: TaskScheduleType
  action: TaskActionType
  concurrency_policy?: ConcurrencyPolicy
  misfire_policy?: MisfirePolicy
  timeout_secs?: number
  is_enabled?: boolean
}

export interface UpdateTaskDto {
  name?: string
  description?: string
  schedule?: TaskScheduleType
  action?: TaskActionType
  concurrency_policy?: ConcurrencyPolicy
  misfire_policy?: MisfirePolicy
  timeout_secs?: number
  is_enabled?: boolean
}

export interface HistoryQueryDto {
  task_id?: string
  limit?: number
  offset?: number
}

export interface PruneHistoryResponse {
  success: boolean
  deleted_count: number
}

export const schedulerApi = {
  getTasks: async (): Promise<ScheduledTask[]> => {
    return api.get<ScheduledTask[]>('/api/v1/system/scheduler/tasks')
  },

  getTask: async (id: string): Promise<ScheduledTask> => {
    return api.get<ScheduledTask>(`/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}`)
  },

  createTask: async (dto: CreateTaskDto): Promise<ScheduledTask> => {
    return api.post<ScheduledTask>('/api/v1/system/scheduler/tasks', dto)
  },

  updateTask: async (id: string, dto: UpdateTaskDto): Promise<ScheduledTask> => {
    return api.put<ScheduledTask>(`/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}`, dto)
  },

  deleteTask: async (id: string): Promise<void> => {
    return api.delete<void>(`/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}`)
  },

  runTaskNow: async (id: string): Promise<TaskExecutionRecord> => {
    return api.post<TaskExecutionRecord>(`/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}/run`, {})
  },

  toggleTask: async (id: string, isEnabled: boolean): Promise<ScheduledTask> => {
    return api.post<ScheduledTask>(`/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}/toggle`, {
      is_enabled: isEnabled
    })
  },

  getTaskHistory: async (id: string, params?: HistoryQueryDto): Promise<TaskExecutionRecord[]> => {
    const query = new URLSearchParams()
    if (params?.limit) query.set('limit', params.limit.toString())
    if (params?.offset !== undefined) query.set('offset', params.offset.toString())
    const qStr = query.toString()
    return api.get<TaskExecutionRecord[]>(
      `/api/v1/system/scheduler/tasks/${encodeURIComponent(id)}/history${qStr ? `?${qStr}` : ''}`
    )
  },

  getAllHistory: async (params?: HistoryQueryDto): Promise<TaskExecutionRecord[]> => {
    const query = new URLSearchParams()
    if (params?.task_id) query.set('task_id', params.task_id)
    if (params?.limit) query.set('limit', params.limit.toString())
    if (params?.offset !== undefined) query.set('offset', params.offset.toString())
    const qStr = query.toString()
    return api.get<TaskExecutionRecord[]>(`/api/v1/system/scheduler/history${qStr ? `?${qStr}` : ''}`)
  },

  pruneHistory: async (days = 30): Promise<PruneHistoryResponse> => {
    return api.delete<PruneHistoryResponse>(`/api/v1/system/scheduler/history?days=${days}`)
  }
}
