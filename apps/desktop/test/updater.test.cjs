const { test } = require('node:test')
const assert = require('node:assert/strict')
const path = require('node:path')
const { execFileSync } = require('node:child_process')
const { createHash, generateKeyPairSync, randomBytes, sign } = require('node:crypto')
const { findFile } = require('electron-updater/out/providers/Provider')
const { SignedProvider } = require('../electron/updater.cjs')
const b64 = (...parts) => Buffer.concat(parts.map(p => Buffer.from(p))).toString('base64')
// A legacy minisign signature, the format deploy/sign-electron-manifests.sh publishes.
function signer() {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519'), keynum = randomBytes(8), trusted = 'timestamp:1'
  return {
    key: b64('Ed', keynum, publicKey.export({ format: 'der', type: 'spki' }).subarray(-32)),
    sign(bytes) {
      const sig = sign(null, bytes, privateKey)
      return `untrusted comment: test\n${b64('Ed', keynum, sig)}\ntrusted comment: ${trusted}\n${b64(sign(null, Buffer.concat([sig, Buffer.from(trusted)]), privateKey))}\n`
    },
  }
}
// Verify in Electron's runtime, as the app does. Plain Node has BLAKE2b and Electron's
// BoringSSL does not, which is how prehashed signatures shipped in 0.3.0 unverifiable.
function verifyInElectron(cases) {
  const script = `const { verifyManifest } = require(${JSON.stringify(path.resolve(__dirname, '../electron/updater.cjs'))})
    Promise.all(JSON.parse(process.env.CASES).map(c => verifyManifest(Buffer.from(c.bytes), c.signature, c.key || undefined)
      .then(info => info.version, e => e.message))).then(r => console.log(JSON.stringify(r)))`
  const out = execFileSync(require('electron'), ['-e', script], { env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', CASES: JSON.stringify(cases) }, encoding: 'utf8' })
  return JSON.parse(out)
}
const manifest = (url, version = '0.3.2') => JSON.stringify({ version, files: [{ url, sha512: createHash('sha512').update('installer').digest('base64'), size: 9 }] })
test('updater accepts a signed manifest inside Electron and rejects changed metadata, bad files, or another key', () => {
  const s = signer(), good = manifest('Den-0.3.2-win-x64.exe'), bad = manifest('../Den.exe')
  assert.deepEqual(verifyInElectron([
    { bytes: good, signature: s.sign(Buffer.from(good)), key: s.key },
    { bytes: good.replace('0.3.2', '9.9.9'), signature: s.sign(Buffer.from(good)), key: s.key },
    { bytes: bad, signature: s.sign(Buffer.from(bad)), key: s.key },
    { bytes: good, signature: s.sign(Buffer.from(good)) },
  ]), ['0.3.2', 'Update signature verification failed', 'Invalid update file', 'Update signature verification failed'])
})
test('a verified manifest resolves to the installer on its own release', () => {
  const info = JSON.parse(manifest('Den-0.3.2-win-x64.exe'))
  const file = findFile(new SignedProvider(null, null, { platform: 'win32' }).resolveFiles(info), 'exe')
  assert.equal(file.url.href, 'https://github.com/nhclink16/den/releases/download/v0.3.2/Den-0.3.2-win-x64.exe')
  assert.equal(file.info.sha512, info.files[0].sha512)
})
