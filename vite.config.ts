import { fileURLToPath } from 'node:url'
import babel from '@rolldown/plugin-babel'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

/** Fixed port Tauri waits on, see `build.devUrl` in src-tauri/tauri.conf.json. */
const port = 1420

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    // React Compiler memoizes components and values, so a parent re-render no longer rebuilds children whose props held
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
  ],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  clearScreen: false,
  server: { port, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: { target: 'es2023' },
})
