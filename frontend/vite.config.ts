import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src')
    }
  },
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    globals: true,
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined;
          }

          if (
            id.includes('/react/') ||
            id.includes('/react-dom/') ||
            id.includes('/scheduler/')
          ) {
            return 'vendor-react';
          }

          if (
            id.includes('/@tanstack/')
          ) {
            return 'vendor-query';
          }

          if (
            id.includes('/react-router/') ||
            id.includes('/react-router-dom/')
          ) {
            return 'vendor-router';
          }

          if (
            id.includes('/i18next/') ||
            id.includes('/react-i18next/')
          ) {
            return 'vendor-i18n';
          }

          if (id.includes('/hls.js/')) {
            return 'vendor-hls';
          }

          return 'vendor-misc';
        },
      },
    },
  },
  server: {
    port: 5173,
    host: true,
    allowedHosts: true,
    // On Windows bind-mounts into a Linux container, native fs events don't
    // propagate, so HMR silently stops refreshing. Enable polling only when
    // explicitly requested to keep local (host) dev unchanged.
    watch: process.env.VITE_WATCH_POLLING
      ? { usePolling: true, interval: 300 }
      : undefined,
    proxy: {
      '/api': {
        target: process.env.VITE_BACKEND_URL || 'http://127.0.0.1:3033',
        changeOrigin: true,
      },
      '/ws': {
        target: (process.env.VITE_BACKEND_URL || 'http://127.0.0.1:3033').replace(/^http/, 'ws'),
        ws: true,
      }
    }
  }
})
