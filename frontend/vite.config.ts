import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    host: true,
    proxy: {
      '/api/v1': {
        target: process.env.VITE_API_BASE_URL?.replace('/api/v1', '') ?? 'http://127.0.0.1:3456',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
  },
})
