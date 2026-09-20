// Background blur runs MediaPipe's selfie segmenter. @livekit/track-processors
// loads its wasm from a public CDN by default, which is wrong for a self-hosted
// Den: the app would phone out to jsdelivr and storage.googleapis.com on every
// camera. Den serves its own copies from /blur instead.
//
// The model is 244 KB and lives in git next to the vendored font. The wasm is
// 19 MB across four files, so it is staged here from node_modules at build time
// rather than committed. Run before `vite dev` or `vite build`; both do.
import { cpSync, existsSync, mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const web = new URL('../apps/web/', import.meta.url)
const candidates = [new URL('node_modules/@mediapipe/tasks-vision/wasm/', web), new URL('../node_modules/@mediapipe/tasks-vision/wasm/', web)]
const from = candidates.find((url) => existsSync(url))
if (!from) {
  console.error('MediaPipe wasm is missing. Run npm install in apps/web.')
  process.exit(1)
}
const to = new URL('public/blur/wasm-0.10.14/', web)
mkdirSync(to, { recursive: true })
cpSync(fileURLToPath(from), fileURLToPath(to), { recursive: true })
console.log(`Staged background blur wasm into ${fileURLToPath(to)}`)
