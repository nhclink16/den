const { test } = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const { tmpdir } = require('node:os')
const path = require('node:path')
const { execFileSync } = require('node:child_process')
const { createHash } = require('node:crypto')
const { verifyManifest } = require('../electron/updater.cjs')
test('updater accepts a signed manifest and rejects changed metadata or another signing key', async () => {
  const directory = fs.mkdtempSync(path.join(tmpdir(), 'den-update-'))
  const cli = path.resolve(__dirname, '../node_modules/@tauri-apps/cli/tauri.js')
  const key = path.join(directory, 'key'), manifest = path.join(directory, 'manifest.json')
  // The release job exports the real signing key; the signer refuses it alongside -f.
  const { TAURI_SIGNING_PRIVATE_KEY, TAURI_SIGNING_PRIVATE_KEY_PASSWORD, ...env } = process.env
  const run = args => execFileSync(process.execPath, [cli, 'signer', ...args], { stdio: 'pipe', env: { ...env, CI: 'true' } })
  try {
    run(['generate', '--ci', '-p', '', '-w', key])
    const publicKey = Buffer.from(fs.readFileSync(key + '.pub', 'utf8').trim(), 'base64').toString().trim().split('\n').at(-1)
    const bytes = Buffer.from(JSON.stringify({ version: '0.2.3', files: [{ url: 'Den-0.2.3-linux-x86_64.AppImage', sha512: createHash('sha512').update('test installer').digest('base64'), size: 14 }] }))
    fs.writeFileSync(manifest, bytes); run(['sign', '-f', key, '-p', '', manifest])
    const signature = Buffer.from(fs.readFileSync(manifest + '.sig', 'utf8').trim(), 'base64').toString()
    assert.equal((await verifyManifest(bytes, signature, publicKey)).version, '0.2.3')
    await assert.rejects(verifyManifest(Buffer.from(bytes.toString().replace('0.2.3', '9.9.9')), signature, publicKey))
    await assert.rejects(verifyManifest(bytes, signature))
  } finally { fs.rmSync(directory, { recursive: true, force: true }) }
})
