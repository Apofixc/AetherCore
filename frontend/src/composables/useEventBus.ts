/**
 * # Vue 3 Composable: `useEventBus`
 *
 * Предоставляет удобную реактивную обёртку над событийной шиной ядра для
 * использования внутри компонентов и динамических представлений плагинов.
 */

import { onMounted, onUnmounted, ref } from 'vue'
import { eventBus, type EventHandler } from '../services/eventBus'
import type { EventMessage, EventPriority } from '../services/types'

export interface UseEventBusOptions {
  /**
   * Топики для автоматической подписки (строка или массив)
   */
  topics?: string | string[]
  /**
   * Обработчик входящих событий
   */
  onEvent?: EventHandler
  /**
   * Автоматически подключаться при монтировании компонента
   */
  autoConnect?: boolean
}

export function useEventBus(options?: UseEventBusOptions) {
  const isConnected = ref(eventBus.isConnected)
  let unsubscribeFn: (() => void) | null = null

  onMounted(() => {
    if (options?.autoConnect !== false) {
      eventBus.connect()
    }

    if (options?.topics && options?.onEvent) {
      unsubscribeFn = eventBus.subscribe(options.topics, options.onEvent)
    }
  })

  onUnmounted(() => {
    if (unsubscribeFn) {
      unsubscribeFn()
      unsubscribeFn = null
    }
  })

  return {
    isConnected,
    /**
     * Подписаться на топики
     */
    subscribe: (topics: string | string[], handler: EventHandler) =>
      eventBus.subscribe(topics, handler),
    /**
     * Опубликовать событие в шину ядра
     */
    publish: (topic: string, payload: any, priority?: EventPriority, retain?: boolean) =>
      eventBus.publish(topic, payload, priority, retain),
    /**
     * Вызвать любой REST метод ядра через открытый сокет (с HTTP Fallback)
     */
    call: <T = any>(method: string, path: string, body?: any): Promise<T> =>
      eventBus.call<T>(method, path, body),
    /**
     * Запросить сохраненный срез состояний
     */
    getState: (patterns: string[], limitPerTopic?: number) =>
      eventBus.getState(patterns, limitPerTopic),
    /**
     * Принудительно переподключиться
     */
    connect: () => eventBus.connect(),
    /**
     * Отключиться
     */
    disconnect: () => eventBus.disconnect()
  }
}
