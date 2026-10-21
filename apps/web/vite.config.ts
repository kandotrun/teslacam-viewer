import { fileURLToPath } from 'node:url'

import { cloudflare } from '@cloudflare/vite-plugin'
import react from '@vitejs/plugin-react'
import { defineConfig, searchForWorkspaceRoot } from 'vite'

// The shared contract and fonts live in the repository root (packages/*), outside the app root.
const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))

export default defineConfig({
  plugins: [react(), cloudflare()],
  server: {
    fs: {
      allow: [searchForWorkspaceRoot(process.cwd()), repositoryRoot],
    },
  },
})
