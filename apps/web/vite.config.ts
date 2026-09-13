import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// The API lives at root paths on den-server. In dev, proxy those so the browser
// origin is the Vite origin and cookies + CSRF just work. Run the server with
// DEN_ORIGIN=http://localhost:5173.
const api = ['hosts', 'requests', 'grants', 'access', 'sessions', 'objects', 'auth', 'users', 'invites', 'tokens', 'bots', 'channels', 'dms', 'categories', 'messages', 'uploads', 'health', 'openapi.json', 'search', 'presence', 'calls', 'livekit']

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: Object.fromEntries([
      ['/install-host.sh', { target: 'http://127.0.0.1:7000' }],
      ['/install-host.ps1', { target: 'http://127.0.0.1:7000' }],
      ['/hosts/ws', { target: 'ws://127.0.0.1:7000', ws: true }],
      ...api.map((p) => [`/${p}`, { target: 'http://127.0.0.1:7000' }]),
      ['^/settings$', { target: 'http://127.0.0.1:7000', bypass: (req: { headers: { accept?: string }; url?: string }) => req.headers.accept?.includes('text/html') ? req.url : undefined }],
      ['/ws', { target: 'ws://127.0.0.1:7000', ws: true }],
    ]),
  },
})
