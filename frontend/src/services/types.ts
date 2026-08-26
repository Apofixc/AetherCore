/**
 * # Типы протокола WebSocket-шлюза AetherCore
 */

export type EventPriority = 'Critical' | 'High' | 'Normal' | 'Low'
export type EventType = 'Telemetry' | 'Reliable'

export interface EventMessage {
  id: string
  topic: string
  event_type: EventType
  priority: EventPriority
  source: string
  payload: any
  binary_payload?: number[] | null
  dedup_key?: string | null
  retain: boolean
  timestamp: string
  expires_at?: string | null
  correlation_id?: string | null
  reply_to?: string | null
}

export type WsClientCommand =
  | { action: 'auth'; token: string }
  | { action: 'subscribe'; topics: string[]; with_retained?: boolean }
  | { action: 'unsubscribe'; topics: string[] }
  | {
      action: 'publish'
      msg_id?: string
      tab_id?: string
      topic: string
      payload: any
      priority?: EventPriority
      retain?: boolean
    }
  | {
      action: 'call'
      request_id: string
      tab_id?: string
      method: string
      path: string
      body?: any
    }
  | { action: 'get_state'; patterns: string[]; limit_per_topic?: number }
  | { action: 'ping' }

export type WsServerMessage =
  | {
      type: 'authenticated'
      user_id: string
      username: string
      roles: string[]
      permissions: string[]
    }
  | {
      type: 'event'
      seq: number
      event: EventMessage
    }
  | {
      type: 'ack'
      msg_id: string
      status: string
    }
  | {
      type: 'response'
      request_id: string
      tab_id?: string
      status: number
      body: any
    }
  | {
      type: 'state_snapshot'
      events: EventMessage[]
    }
  | {
      type: 'subscribed'
      topics: string[]
    }
  | {
      type: 'unsubscribed'
      topics: string[]
    }
  | {
      type: 'pong'
    }
  | {
      type: 'error'
      code: string
      message: string
      request_id?: string
    }
