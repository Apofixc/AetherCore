export interface ApiResponse<T = any> {
  success?: boolean
  data?: T
  error?: string
  message?: string
  [key: string]: any
}

class ApiClient {
  private get token(): string | null {
    return localStorage.getItem('nms_token')
  }

  public setToken(token: string | null) {
    if (token) {
      localStorage.setItem('nms_token', token)
    } else {
      localStorage.removeItem('nms_token')
    }
  }

  public async request<T = any>(endpoint: string, options: RequestInit = {}): Promise<T> {
    const headers = new Headers(options.headers || {})
    
    if (!headers.has('Content-Type') && !(options.body instanceof FormData)) {
      headers.set('Content-Type', 'application/json')
    }

    if (this.token && !headers.has('Authorization')) {
      headers.set('Authorization', `Bearer ${this.token}`)
    }

    const response = await fetch(endpoint, {
      ...options,
      headers
    })

    if (response.status === 401 && !endpoint.includes('/auth/login') && !endpoint.includes('/auth/config')) {
      // Токен истек или отсутствует для защищенного маршрута
      this.setToken(null)
      if (window.location.pathname !== '/login') {
        window.location.href = '/login'
      }
    }

    let data: any
    const contentType = response.headers.get('Content-Type') || ''
    if (contentType.includes('application/json')) {
      data = await response.json()
    } else {
      data = await response.text()
    }

    if (!response.ok) {
      const errorMsg =
        data?.error?.message ||
        (typeof data?.error === 'string' ? data.error : null) ||
        data?.message ||
        `HTTP ${response.status}: ${response.statusText}`
      throw new Error(errorMsg)
    }

    return data as T
  }

  public get<T = any>(endpoint: string, options?: RequestInit) {
    return this.request<T>(endpoint, { ...options, method: 'GET' })
  }

  public post<T = any>(endpoint: string, body?: any, options?: RequestInit) {
    return this.request<T>(endpoint, {
      ...options,
      method: 'POST',
      body: body instanceof FormData ? body : JSON.stringify(body)
    })
  }

  public put<T = any>(endpoint: string, body?: any, options?: RequestInit) {
    return this.request<T>(endpoint, {
      ...options,
      method: 'PUT',
      body: body instanceof FormData ? body : JSON.stringify(body)
    })
  }

  public delete<T = any>(endpoint: string, options?: RequestInit) {
    return this.request<T>(endpoint, { ...options, method: 'DELETE' })
  }
}

export const api = new ApiClient()
