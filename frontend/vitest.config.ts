import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import path from 'path'

export default defineConfig({
  // 公共目录的绝对 URL 无需转成模块导入，避免 Windows 生成无盘符的 file URL。
  plugins: [vue({ template: { transformAssetUrls: { includeAbsolute: false } } })],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/tests/vitest.setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'dist/',
        '**/*.spec.ts',
        '**/*.d.ts',
        '**/vite.config.ts'
      ]
    }
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
})
