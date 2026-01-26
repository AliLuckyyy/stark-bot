import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src')
    }
  },
  server: {
    port: 8080,
    proxy: {
      '/api': 'http://backend:8082',
      '/ws': {
        target: 'ws://backend:8081',
        ws: true,
        changeOrigin: true,
        rewritePath: (path) => path.replace(/^\/ws/, '')
      }
    }
  }
});
