import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'
import Markdown from 'vite-plugin-react-markdown'

export default defineConfig({
  plugins: [
    Markdown({
      wrapperComponentPath: './src/views/docs/DocsWrapper',
    }),
    react({
      include: [/\.tsx$/, /\.md$/],
    }),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 3000,
  },
})
