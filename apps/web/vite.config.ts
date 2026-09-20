import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, transformWithOxc } from 'vite'
import { readFileSync } from 'node:fs'

// The API lives at root paths on den-server. In dev, proxy those so the browser
// origin is the Vite origin and cookies + CSRF just work. Run the server with
// DEN_ORIGIN=http://127.0.0.1:5173 and browse to http://127.0.0.1:5173.
//
// Not localhost. Spotify refuses `localhost` as a redirect host and only the
// registered `http://127.0.0.1:5173/spotify/callback` works, while a browser
// treats localhost and 127.0.0.1 as different origins — so mixing the two breaks
// the OAuth callback, the session cookie, or both.
//
// `spotify` is deliberately absent from the list below: /spotify/callback is an
// SPA route that must reach index.html, not the API.
const api = ['rooms', 'instance', 'hosts', 'requests', 'grants', 'access', 'sessions', 'objects', 'auth', 'users', 'invites', 'tokens', 'bots', 'channels', 'dms', 'categories', 'messages', 'uploads', 'health', 'openapi.json', 'search', 'presence', 'calls', 'livekit']
const apiTarget = process.env.DEN_API_TARGET || 'http://127.0.0.1:7000'
const wsTarget = apiTarget.replace(/^http/, 'ws')

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
      ['/install-host.sh', { target: apiTarget }],
      ['/install-host.ps1', { target: apiTarget }],
      ['/hosts/ws', { target: wsTarget, ws: true }],
      ...api.map((p) => [`/${p}`, { target: apiTarget }]),
      ['^/settings(?:/sounds(?:/files/[^/]+)?)?$', { target: apiTarget, bypass: (req: { headers: { accept?: string }; url?: string }) => req.headers.accept?.includes('text/html') ? req.url : undefined }],
      ['/ws', { target: wsTarget, ws: true }],
    ]),
  },
})
