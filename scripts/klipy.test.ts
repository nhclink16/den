import assert from 'node:assert/strict'
import { test } from 'node:test'
import { klipyGif, snippet } from '../apps/web/src/lib/klipy.ts'

test('only a message that is nothing but a KLIPY GIF link is drawn as a GIF', () => {
  for (const url of [
    'https://static.klipy.com/ii/abc/84/09/44ftKeij.webp',
    'https://static1.klipy.com/ii/abc/x.gif',
    '  https://static2.klipy.com/ii/abc/x.webp\n',
  ]) assert.equal(klipyGif(url), url.trim())
  for (const text of [
    'look https://static.klipy.com/ii/abc/x.webp',
    'https://static.klipy.com/ii/abc/x.webp lol',
    'http://static.klipy.com/ii/abc/x.webp',
    'https://static.klipy.com.evil.example/x.webp',
    'https://evil.example/static.klipy.com/x.webp',
    'https://static.klipy.com/ii/abc/x.mp4',
    'https://klipy.com/gifs/hello',
  ]) assert.equal(klipyGif(text), null, text)
  assert.equal(snippet('https://static.klipy.com/ii/abc/x.webp'), 'GIF')
  assert.equal(snippet('hello'), 'hello')
})
