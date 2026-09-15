const { test } = require('node:test')
const assert = require('node:assert/strict')
const { mkdtempSync, readFileSync, rmSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { join } = require('node:path')
const { createServer } = require('node:http')
const { origin, requestUrl, storage, apiRequest } = require('../electron/session.cjs')
test('native transport rejects origin escapes and credential injection', async () => {
  for (const value of ['http://example.com', 'https://user:secret@example.com', 'https://example.com/path', 'https://example.com/?secret=1']) assert.throws(() => origin(value))
  for (const value of ['//evil.test/x', '/\\evil.test/x', 'https://evil.test/x']) assert.throws(() => requestUrl('https://example.com', value))
  let received
  const server = createServer((req, res) => { received = req.headers; if (req.url === '/redirect') { res.writeHead(302, { location: 'https://example.com' }); res.end() } else { res.setHeader('set-cookie', 'secret'); res.end('{}') } })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  const base = `http://127.0.0.1:${server.address().port}`
  try {
    const response = await apiRequest({ get: () => 'native-secret' }, { origin: base, method: 'GET', path: '/users/me', headers: { authorization: 'injected', cookie: 'injected', range: 'bytes=0-2' } })
    assert.equal(received.authorization, 'Bearer native-secret'); assert.equal(received.cookie, undefined); assert.equal(received.range, 'bytes=0-2'); assert.equal(response.headers['set-cookie'], undefined)
    await assert.rejects(apiRequest({ get: () => 'secret' }, { origin: base, method: 'GET', path: '/redirect' }), /Cannot reach/)
  } finally { await new Promise(resolve => server.close(resolve)) }
})
test('keychain failures refuse writes; encrypted sessions are isolated by origin', () => {
  const directory = mkdtempSync(join(tmpdir(), 'den-storage-'))
  let available = false
  const safe = { isEncryptionAvailable: () => available, getSelectedStorageBackend: () => 'gnome_libsecret', encryptString: s => Buffer.from(s).reverse(), decryptString: b => Buffer.from(b).reverse().toString() }
  try {
    const store = storage(directory, safe)
    assert.throws(() => store.set('https://one.test', 'first-secret'), /keychain/)
    available = true; store.set('https://one.test', 'first-secret'); store.set('https://two.test', 'second-secret')
    assert(!readFileSync(join(directory, 'native.json'), 'utf8').includes('first-secret'))
    const restored = storage(directory, safe); assert.equal(restored.get('https://one.test'), 'first-secret'); restored.clear('https://one.test'); assert.equal(restored.get('https://one.test'), null); assert.equal(restored.get('https://two.test'), 'second-secret')
    if (process.platform === 'linux') { safe.getSelectedStorageBackend = () => 'basic_text'; assert.throws(() => restored.get('https://two.test'), /keychain/) }
  } finally { rmSync(directory, { recursive: true }) }
})
