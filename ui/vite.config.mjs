import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 3000,
    strictPort: true,
    // Exclude the Tauri target directory from file watching to prevent EBUSY errors
    // when the Tauri binary is being rebuilt.
    watch: {
      ignored: ['**/src-tauri/target/**', '**/node_modules/**']
    }
  },
  preview: {
    port: 4173
  },
  optimizeDeps: {
    exclude: ['src-tauri/target']
  }
})
