import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 3000,
    strictPort: true
  },
  preview: {
    port: 4173
  }
})