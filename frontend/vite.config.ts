import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
    watch: {
      usePolling: true,
      interval: 100,
    },
    hmr: {
      overlay: true,
    },
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (_proxyReq, req) => {
            console.log(`\x1b[36m[Vite Proxy ->]\x1b[0m ${req.method} ${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            const status = proxyRes.statusCode || 200
            const color = status >= 400 ? '\x1b[31m' : '\x1b[32m'
            console.log(`\x1b[36m[Vite Proxy <-]\x1b[0m ${req.method} ${req.url} -> ${color}${status}\x1b[0m`)
          })
          proxy.on('error', (err, req) => {
            console.error(`\x1b[31m[Vite Proxy Error]\x1b[0m ${req.method} ${req.url}:`, err.message)
          })
        }
      },
      '/ws': {
        target: 'ws://127.0.0.1:3000',
        ws: true,
        configure: (proxy) => {
          proxy.on('open', () => {
            console.log('\x1b[35m[Vite WS Proxy]\x1b[0m WebSocket connection opened to backend')
          })
          proxy.on('error', (err) => {
            console.error('\x1b[31m[Vite WS Proxy Error]\x1b[0m', err.message)
          })
        }
      },
      '/modules': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: true
      }
    }
  }
})
