// SPDX-License-Identifier: AGPL-3.0-or-later
import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import { copyFileSync, mkdirSync } from 'node:fs'

/** web-ifc's WebAssembly is served from /wasm/ (copied from node_modules, never committed). */
function webIfcWasm(): Plugin {
  return {
    name: 'web-ifc-wasm',
    buildStart() {
      mkdirSync('public/wasm', { recursive: true })
      copyFileSync('node_modules/web-ifc/web-ifc.wasm', 'public/wasm/web-ifc.wasm')
    },
  }
}

const server = process.env.NICGIT_DEV_SERVER ?? 'http://127.0.0.1:3000'

export default defineConfig({
  plugins: [react(), webIfcWasm()],
  server: { proxy: { '/api': server } },
  build: { chunkSizeWarningLimit: 2000 },
})
