import { api } from './client'

export interface ReliableEventRecord {
  id: number
  timestamp: string
  topic: string
  payload: Record<string, any>
  source_plugin?: string
}

export interface EventsQueryParams {
  topic?: string
  after_id?: number
  limit?: number
}

export const eventsApi = {
  query: async (params?: EventsQueryParams): Promise<ReliableEventRecord[]> => {
    const query = new URLSearchParams()
    if (params?.topic) query.set('topic', params.topic)
    if (params?.after_id) query.set('after_id', params.after_id.toString())
    if (params?.limit) query.set('limit', params.limit.toString())
    const qStr = query.toString()
    return api.get<ReliableEventRecord[]>(`/api/v1/events${qStr ? `?${qStr}` : ''}`)
  }
}
