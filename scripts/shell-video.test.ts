import { test, type TestContext } from 'node:test'
import assert from 'node:assert/strict'
import { holes, nativeVideo, videoKey, type ShellVideoCommand, type ShellVideoReply, type ShellVideoEvent } from '../apps/web/src/lib/shell-video.ts'

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

// Exercise the real placement/acknowledgement state machine with a tiny DOM and a
// manually clocked shell. No camera, browser, server or wall-clock timer is needed.
function scene(t: TestContext) {
  const frames = new Map<number, FrameRequestCallback>()
  const requests: { command: ShellVideoCommand; answer: (r: ShellVideoReply) => void }[] = []
  const listeners = new Map<string, EventListener>()
  let nextFrame = 0
  const style = () => {
    const values = new Map<string, string>()
    return {
      getPropertyValue: (key: string) => values.get(key) ?? '',
      setProperty: (key: string, value: string) => { values.set(key, value) },
      removeProperty: (key: string) => { values.delete(key) },
    }
  }
  const body = { style: style() }
  const root = { dataset: {} as Record<string, string> }
  const document = {
    body, documentElement: root, visibilityState: 'visible',
    querySelectorAll: () => [], addEventListener() {}, removeEventListener() {},
  }
  const bridge: { video: (c: ShellVideoCommand) => Promise<ShellVideoReply>; onVideo?: (e: ShellVideoEvent) => void } = {
    video: command => new Promise(answer => { requests.push({ command, answer }) }),
  }
  const original = new Map<string, PropertyDescriptor | undefined>()
  const globals: Record<string, unknown> = {
    document, window: { denShell: bridge }, innerWidth: 402, innerHeight: 874,
    requestAnimationFrame: (callback: FrameRequestCallback) => { frames.set(++nextFrame, callback); return nextFrame },
    cancelAnimationFrame: (id: number) => { frames.delete(id) },
    setInterval: () => 1, clearInterval() {},
    MutationObserver: class { observe() {} disconnect() {} },
    ResizeObserver: class { observe() {} unobserve() {} disconnect() {} },
    addEventListener: (name: string, fn: EventListener) => { listeners.set(name, fn) },
    removeEventListener: (name: string) => { listeners.delete(name) },
    getComputedStyle: () => ({ borderTopLeftRadius: '14px', backgroundColor: 'rgb(40, 40, 44)' }),
  }
  for (const [key, value] of Object.entries(globals)) {
    original.set(key, Object.getOwnPropertyDescriptor(globalThis, key))
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value })
  }
  const node = {
    isConnected: true, parentElement: body, dataset: {} as Record<string, string>,
    rect: { ...tile.rect }, getBoundingClientRect() { return this.rect },
  }
  const action = nativeVideo(node as unknown as HTMLElement, { identity: 'u1:a', source: 'camera', fit: 'cover' })
  let destroyed = false
  const destroy = () => { if (!destroyed) { action.destroy(); destroyed = true } }
  t.after(() => {
    try { destroy() } finally {
      for (const [key, descriptor] of original) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor)
        else Reflect.deleteProperty(globalThis, key)
      }
    }
  })
  return {
    node, root, body, requests, bridge, destroy,
    scroll: () => listeners.get('scroll')?.({} as Event),
    frame: () => {
      const callbacks = [...frames.values()]; frames.clear()
      for (const callback of callbacks) callback(0)
    },
  }
}

test('native video waits for the newest placement and ignores stale readiness', async t => {
  const s = scene(t)
  assert.equal(s.node.dataset.native, 'off')
  assert.equal(s.requests.length, 0, 'a moving tile is not sent until the next frame')
  s.frame()
  const first = s.requests[0]
  s.node.rect = { ...s.node.rect, y: 200 }
  s.scroll(); s.frame(); s.frame()
  const second = s.requests[1]
  assert(second.command.seq > first.command.seq)
  const rev = second.command.seq * 10
  first.answer({ seq: first.command.seq, rev: rev - 1, ready: ['u1:a|camera'] })
  await Promise.resolve()
  assert.equal(s.node.dataset.native, 'off', 'an old placement must not open the moved tile')
  second.answer({ seq: second.command.seq, rev, ready: ['u1:a|camera'] })
  await Promise.resolve()
  assert.equal(s.node.dataset.native, 'on')
  s.bridge.onVideo?.({ type: 'ready', seq: second.command.seq, rev: rev + 1, ready: [] })
  assert.equal(s.node.dataset.native, 'off', 'a missing video closes its hole')
  s.bridge.onVideo?.({ type: 'ready', seq: second.command.seq, rev, ready: ['u1:a|camera'] })
  assert.equal(s.node.dataset.native, 'off', 'an older readiness reply cannot bring it back')
})

test('scroll closes video immediately and destroying its last slot clears the shell', async t => {
  const s = scene(t)
  s.frame()
  const first = s.requests[0]
  const rev = first.command.seq * 10
  first.answer({ seq: first.command.seq, rev, ready: ['u1:a|camera'] })
  await Promise.resolve()
  assert.equal(s.node.dataset.native, 'on')
  s.node.rect = { ...s.node.rect, y: 220 }
  s.scroll()
  assert.equal(s.node.dataset.native, 'off', 'scroll closes the hole before the next animation frame')
  s.frame(); s.frame()
  const pending = s.requests[1]
  s.destroy()
  const cleared = s.requests[2].command
  assert(cleared.seq > pending.command.seq)
  assert.deepEqual(cleared.slots, [])
  assert.equal(s.node.dataset.native, undefined)
  assert.equal(s.root.dataset.nativeVideo, undefined)
  assert.equal(s.body.style.getPropertyValue('--den-holes'), '')
  pending.answer({ seq: pending.command.seq, rev: rev + 1, ready: ['u1:a|camera'] })
  await Promise.resolve()
  assert.equal(s.node.dataset.native, undefined, 'a late reply cannot restore a destroyed slot')
  assert.equal(s.body.style.getPropertyValue('--den-holes'), '')
})

