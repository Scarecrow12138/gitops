import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  // Tauri expects a fixed port; fail if it's not available
  server: {
    port: 5173,
    strictPort: true,
  },
  // Prevent vite from obscuring rust errors
  clearScreen: false,
})
