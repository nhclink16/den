import { test } from 'node:test'
import assert from 'node:assert/strict'
import { holes, videoKey } from '../apps/web/src/lib/shell-video.ts'

// Den Shell draws call video behind the page, which cuts holes for it: the names must match the shell's own
// (CallEngine.swift `CallRules.videoKey`), and the holes must land on the tiles in each layer's own coordinates.
test('videos are named by whose they are and which camera or share', () => {
  assert.equal(videoKey({ identity: 'u1:a', source: 'camera', fit: 'cover' }), 'u1:a|camera')
  assert.equal(videoKey({ identity: 'u1:a', source: 'screen', name: 'screen-2', fit: 'contain' }), 'u1:a|screen|screen-2')
})

const svg = (mask: string) => decodeURIComponent(mask.slice('url("data:image/svg+xml,'.length, -2))
const tile = { rect: { x: 12, y: 100, width: 378, height: 212 }, visible: { x: 12, y: 100, width: 378, height: 212 }, radius: 14 }

test('no open tile, no mask: the layer is drawn whole, as without the shell', () => {
  assert.equal(holes({ x: 0, y: 0, width: 402, height: 874 }, []), 'none')
})

test('a hole is the tile, its corners, and only the part of it on screen', () => {
  const cut = { ...tile, visible: { x: 12, y: 150, width: 378, height: 162 } }
  const s = svg(holes({ x: 0, y: 0, width: 402, height: 874 }, [cut]))
  assert.match(s, /^<svg xmlns="http:\/\/www.w3.org\/2000\/svg" width="402" height="874">/)
  assert.match(s, /<clipPath id="c0"><rect x="12" y="150" width="378" height="162"\/><\/clipPath>/)
  assert.match(s, /<rect x="12" y="100" width="378" height="212" rx="14" ry="14" clip-path="url\(#c0\)"\/>/)
})

test('a wallpaper layer scaled about its centre gets the hole where the tile is on screen', () => {
  // The wallpaper is scale(1.06) over a 402×874 box at 0,0: a point p on screen is 201 + (p - 201) / 1.06 inside it.
  const s = svg(holes({ x: 0, y: 0, width: 402, height: 874 }, [tile], 1.06, 1.06))
  const x = Math.round((201 + (12 - 201) / 1.06) * 100) / 100, y = Math.round((437 + (100 - 437) / 1.06) * 100) / 100
  assert.match(s, new RegExp(`<rect x="${x}" y="${y}" width="356.6" height="200" rx="13.21" ry="13.21"`))
  // A layer that starts lower down (a panel) takes its own origin off.
  assert.match(svg(holes({ x: 0, y: 50, width: 402, height: 824 }, [tile])), /<rect x="12" y="50" width="378" height="212"/)
})
