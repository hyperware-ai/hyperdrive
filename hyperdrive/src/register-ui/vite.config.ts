import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { NodeGlobalsPolyfillPlugin } from '@esbuild-plugins/node-globals-polyfill'
import UnoCSS from '@unocss/vite'

export default defineConfig({
  plugins: [
    NodeGlobalsPolyfillPlugin({
      buffer: true
    }),
    UnoCSS(),
    react(),
  ],
  server: {
    proxy: {
      // TODO fix these
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, '')
      },
    }
  }
})
