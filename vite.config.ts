import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

/** Fixed port Tauri waits on, see `build.devUrl` in src-tauri/tauri.conf.json. */
const port = 1420

export default defineConfig({
  plugins: [tanstackRouter({ target: 'react', autoCodeSplitting: true }), react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  clearScreen: false,
  server: { port, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: { target: 'es2023' },
})
