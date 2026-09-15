import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, transformWithOxc } from 'vite'
import { readFileSync } from 'node:fs'

// The API lives at root paths on den-server. In dev, proxy those so the browser
// origin is the Vite origin and cookies + CSRF just work. Run the server with
// DEN_ORIGIN=http://localhost:5173.
const api = ['rooms', 'instance', 'hosts', 'requests', 'grants', 'access', 'sessions', 'objects', 'auth', 'users', 'invites', 'tokens', 'bots', 'channels', 'dms', 'categories', 'messages', 'uploads', 'health', 'openapi.json', 'search', 'presence', 'calls', 'livekit']

export default defineConfig({
  worker: { format: 'es' },
  plugins: [svelte(), {
    name: 'den-first-paint',
    async transformIndexHtml(html) {
      const source = readFileSync(new URL('./src/lib/theme-runtime.ts', import.meta.url), 'utf8').replace(/^import .*$/gm, '').replace(/export /g, '')
      const builtins = readFileSync(new URL('../../crates/den-core/src/themes.json', import.meta.url), 'utf8')
      const { code } = await transformWithOxc(`const builtins = ${builtins};\n${source}\nfirstPaint()`, 'theme.ts', { lang: 'ts' })
      return html.replace('<!-- appearance-first-paint -->', `<script>${code}</script>`)
    },
  }],
  server: {
    port: 5173,
    strictPort: true,
    proxy: Object.fromEntries([
      ['/install-host.sh', { target: 'http://127.0.0.1:7000' }],
      ['/install-host.ps1', { target: 'http://127.0.0.1:7000' }],
      ['/hosts/ws', { target: 'ws://127.0.0.1:7000', ws: true }],
      ...api.map((p) => [`/${p}`, { target: 'http://127.0.0.1:7000' }]),
      ['^/settings(?:/sounds(?:/files/[^/]+)?)?$', { target: 'http://127.0.0.1:7000', bypass: (req: { headers: { accept?: string }; url?: string }) => req.headers.accept?.includes('text/html') ? req.url : undefined }],
      ['/ws', { target: 'ws://127.0.0.1:7000', ws: true }],
    ]),
  },
})
