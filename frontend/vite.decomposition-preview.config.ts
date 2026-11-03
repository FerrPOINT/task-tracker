import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { createUiProviderDedupe, createUiFontAssetsPlugin } from '@pdlc/ui/build'
import { resolve } from 'node:path'

// Separate entry: no production routes, auth clients, API proxy or live data.
export default defineConfig({
  plugins: [createUiFontAssetsPlugin(), react(), tailwindcss()],
  resolve: { dedupe: createUiProviderDedupe() },
  preview: { host: '127.0.0.1', port: 4186, strictPort: true },
  build: {
    outDir: '.preview-decomposition-dist',
    rollupOptions: { input: resolve(__dirname, 'decomposition-preview.html') },
  },
})
