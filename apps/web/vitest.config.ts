import { configDefaults, defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    environment: 'jsdom',
    exclude: [...configDefaults.exclude, '.worktrees/**', 'tests/e2e/**'],
    globals: true,
    // Stylesheets read as text by tests (`?raw`) must not be stubbed out.
    css: { include: [/\.css(?:\?raw)?$/u] },
    setupFiles: ['./src/test/setup.ts'],
  },
})
