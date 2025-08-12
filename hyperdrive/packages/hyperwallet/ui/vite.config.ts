import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    assetsDir: '.',
    rollupOptions: {
      external: ['/our.js'],
      output: {
        manualChunks: undefined,
      },
    },
  },
  server: {
    port: 3000,
  },
})