import { test } from 'node:test'
import assert from 'node:assert/strict'
import { bounds, overlaps, pack, participantSlots, presetCells, readLayout, storageKey, type TileSpec } from '../apps/web/src/lib/call-layout.ts'
const tiles: TileSpec[] = [{key:'a:screen',kind:'screen'},{key:'b:screen',kind:'screen'},...['a','b','c'].map(key=>({key:key+':cam',kind:'cam' as const}))]
test('shares receive equal large slots and packing reserves a manual drop without overlap', () => {
  const auto=presetCells(tiles,'Auto',{})
  assert.equal(auto['a:screen'].w,6); assert.equal(auto['b:screen'].w,6)
  assert(auto['a:screen'].h>auto['a:cam'].h)
  const moved=pack(tiles,{...auto,'c:cam':{...auto['c:cam'],col:0,row:0}},'c:cam')
  assert.equal(moved['c:cam'].row,0)
  for(const [i,a] of Object.values(moved).entries()) for(const b of Object.values(moved).slice(i+1)) assert(!overlaps(a,b))
  assert.equal(Object.keys(moved).length,tiles.length)
  const without=pack(tiles.filter(t=>t.key!=='c:cam'),auto)
  assert.deepEqual(without['a:screen'],auto['a:screen'])
  assert.deepEqual(bounds({col:11,row:-3,w:99,h:1,pinned:false}),{col:0,row:0,w:12,h:2,pinned:false})
})
test('multiple pins grow while every other tile remains, including on mobile',()=>{
  const auto=presetCells(tiles,'Auto',{})
  auto['a:cam'].pinned=true; auto['b:screen'].pinned=true
  const focus=presetCells(tiles,'Focus',auto)
  assert(focus['a:cam'].w>auto['a:cam'].w)
  assert.equal(focus['a:cam'].w,focus['b:screen'].w)
  assert.equal(Object.keys(focus).length,tiles.length)
  const mobile=presetCells(tiles,'Focus',auto,true)
  assert.equal(mobile['a:cam'].w,12)
  assert.equal(Object.keys(mobile).length,tiles.length)
})
test('rejoining connections reuse vacant saved slots without merging simultaneous devices',()=>{
  const cell={col:0,row:0,w:6,h:4,pinned:true}
  const saved={'user:old:cam':cell,'user:other:cam':cell}
  const slots=participantSlots(['user:new','user:other'],saved)
  assert.equal(slots['user:other'],'user:other');assert.equal(slots['user:new'],'user:old')
  assert.equal(new Set(Object.values(participantSlots(['user:x','user:y','user:z'],saved))).size,3)
})

// Andy reported a rotated monitor leaving most of the call area black. The cause was
// layout, not styling: tiles were laid out across the narrow axis and the row height
// was bounded by the column width, so the rows could never grow into the spare height.
test('a portrait container stacks tiles instead of spreading them across the narrow axis', () => {
  const few: TileSpec[] = [{key:'n:screen',kind:'screen'},{key:'a:cam',kind:'cam'},{key:'n:cam',kind:'cam'}]
  const wide = presetCells(few, 'Auto', {})
  const tall = presetCells(few, 'Auto', {}, true)

  // Landscape keeps two cams beside each other; portrait gives each the full width.
  assert.equal(wide['a:cam'].row, wide['n:cam'].row)
  assert.notEqual(wide['a:cam'].col, wide['n:cam'].col)
  assert.equal(tall['a:cam'].w, 12)
  assert.equal(tall['a:cam'].col, 0)
  assert.equal(tall['n:cam'].col, 0)
  assert.notEqual(tall['a:cam'].row, tall['n:cam'].row)

  // Stacking is what buys the height back, so portrait must occupy more rows.
  const rows = (cells: Record<string, {row:number;h:number}>) => Math.max(...Object.values(cells).map(c => c.row + c.h))
  assert(rows(tall) > rows(wide), 'portrait should use more rows than landscape')

  // A crowd still pairs up: stacking five cams full width would scroll off the screen.
  const crowd: TileSpec[] = ['a','b','c','d','e'].map(k => ({key:`${k}:cam`, kind:'cam' as const}))
  assert.equal(presetCells(crowd, 'Auto', {}, true)['a:cam'].w, 6)

  // Nothing overlaps in either orientation.
  for (const cells of [wide, tall])
    for (const [i, a] of Object.values(cells).entries())
      for (const b of Object.values(cells).slice(i + 1)) assert(!overlaps(a as never, b as never))
})

// Andy's screenshot turned out to be a Custom layout, not a preset, so the portrait
// work in #7 never touched his case: a tile that was a comfortable half of a wide
// window became a sliver of a tall one and stayed that way. Layouts are now stored
// per orientation, so rotating loads the one built for that shape and rotating back
// returns the original untouched.
test('custom layouts are kept per orientation and landscape keeps the original key', () => {
  const store = new Map<string, string>()
  ;(globalThis as { localStorage?: unknown }).localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  }

  // Landscape must keep the key it already used, or everyone loses their layout.
  assert.equal(storageKey('room-1'), 'den.call-layout:room-1')
  assert.notEqual(storageKey('room-1', true), storageKey('room-1'))

  const wide = { preset: 'Custom' as const, tiles: { 'a:cam': { col: 0, row: 0, w: 6, h: 7, pinned: false } } }
  store.set(storageKey('room-1'), JSON.stringify(wide))

  // The landscape arrangement survives.
  assert.equal(readLayout('room-1', false).preset, 'Custom')
  assert.equal(readLayout('room-1', false).tiles['a:cam'].w, 6)

  // Portrait has nothing saved yet, so it starts from a preset rather than
  // inheriting a half-width tile that would be a sliver on a tall screen.
  assert.equal(readLayout('room-1', false, true).preset, 'Auto')
  assert.deepEqual(readLayout('room-1', false, true).tiles, {})

  // Saving in portrait must not disturb landscape.
  const tall = { preset: 'Custom' as const, tiles: { 'a:cam': { col: 0, row: 0, w: 12, h: 5, pinned: false } } }
  store.set(storageKey('room-1', true), JSON.stringify(tall))
  assert.equal(readLayout('room-1', false, true).tiles['a:cam'].w, 12)
  assert.equal(readLayout('room-1', false).tiles['a:cam'].w, 6)
})

// Nicholas, alone with his camera on in Auto, got a 1908x371 strip (about 5:1) with his
// face cropped off: every desktop preset gave cams a fixed three rows whatever their
// width, and a row is at most .8 of a column. Tiles are now sized to the stage instead.
// Stages measured in the app at 1280x800, 1970x1600 (with and without the member
// list) and 1920x1080; the last is a short window.
const stages = [[732, 591], [1422, 1391], [1908, 1391], [1372, 871], [1000, 500]]
const cams = (n: number): TileSpec[] => Array.from({ length: n }, (_, i) => ({ key: `p${i}:cam`, kind: 'cam' as const }))
// What CallGrid does with the cells: pixel boxes, rows squeezed only past the fit.
function render(tiles: TileSpec[], preset: 'Auto' | 'Even' | 'Focus', [width, height]: number[], saved = {}) {
  const stepX = (width + 8) / 12, fit = Math.floor((height - 8) / (stepX * .8))
  const cells = presetCells(tiles, preset, saved, false, fit)
  const rows = Math.max(...Object.values(cells).map(c => c.row + c.h))
  const stepY = Math.max(24, Math.min(stepX * .8, (height - 8) / rows))
  const box = (k: string) => ({ ...cells[k], px: cells[k].w * stepX - 8, ratio: (cells[k].w * stepX - 8) / (cells[k].h * stepY - 8) })
  return { cells, rows, fit, box }
}

test('desktop presets keep cams close to 16:9 and inside the stage', () => {
  for (const stage of stages) for (const n of [1, 2, 3, 4, 6, 9]) for (const preset of ['Auto', 'Even', 'Focus'] as const) {
    const { cells, rows, fit, box } = render(cams(n), preset, stage)
    const at = `${preset}, ${n} cams, ${stage.join('x')}`
    assert(rows <= fit, `${at}: ${rows} rows don't fit in ${fit}`)
    for (const k of Object.keys(cells)) assert(box(k).ratio > 1.6 && box(k).ratio < 2, `${at}: ${k} is ${box(k).ratio.toFixed(2)}:1`)
    // Each line of tiles is centred: as much room left of it as right.
    const lines = Map.groupBy(Object.values(cells), c => c.row)
    for (const line of lines.values()) {
      const left = Math.min(...line.map(c => c.col)), right = 12 - Math.max(...line.map(c => c.col + c.w))
      assert.equal(left, right, `${at}: line at row ${line[0].row} is off centre`)
    }
    for (const [i, a] of Object.values(cells).entries()) for (const b of Object.values(cells).slice(i + 1)) assert(!overlaps(a, b), at)
  }
})

test('one person gets the largest tile that fits, two to four share the stage', () => {
  // A wide stage: the full width. A short one: as wide as the height allows.
  assert.equal(render(cams(1), 'Auto', [1372, 871]).box('p0:cam').w, 12)
  const short = render(cams(1), 'Auto', [1000, 500])
  assert(short.box('p0:cam').w < 12 && short.rows <= short.fit)
  assert.equal(short.box('p0:cam').col, (12 - short.box('p0:cam').w) / 2)
  // Two to four people each get at least half the width, never a thin row of quarters.
  for (const stage of stages.slice(0, 4)) for (const n of [2, 3, 4])
    assert(render(cams(n), 'Auto', stage).box('p0:cam').w >= 6, `${n} cams at ${stage.join('x')}`)
})

test('a share in Auto comes first and large, with cams in shape under it', () => {
  const tiles: TileSpec[] = [{ key: 'p0:screen', kind: 'screen' }, ...cams(3)]
  for (const stage of stages) {
    const { cells, rows, fit, box } = render(tiles, 'Auto', stage)
    assert(rows <= fit, `${stage.join('x')}: ${rows} rows don't fit in ${fit}`)
    assert.equal(cells['p0:screen'].row, 0)
    for (const k of ['p0:cam', 'p1:cam', 'p2:cam']) {
      assert(cells[k].row >= cells['p0:screen'].h, 'cams sit under the share')
      assert(box(k).ratio > 1.6 && box(k).ratio < 2, `${stage.join('x')}: ${k} is ${box(k).ratio.toFixed(2)}:1`)
      assert(box('p0:screen').px > box(k).px, 'the share is larger than a cam')
    }
    assert(box('p0:screen').ratio > 1.6 && box('p0:screen').ratio < 2)
  }
})

// The phone and a portrait monitor were already right; this pins them to what they were.
test('stacked presets are unchanged', () => {
  const flat = (c: Record<string, { col: number; row: number; w: number; h: number }>) =>
    Object.entries(c).map(([k, v]) => `${k.split(':')[1]} ${v.col},${v.row},${v.w},${v.h}`).join(' | ')
  const share: TileSpec = { key: 'p0:screen', kind: 'screen' }
  const expected: Record<string, [TileSpec[], string, string, string]> = {
    '1': [cams(1), 'cam 0,0,12,5', 'cam 0,0,12,5', 'cam 0,0,12,7'],
    '2': [cams(2), 'cam 0,0,12,5 | cam 0,5,12,5', 'cam 0,0,6,5 | cam 6,0,6,5', 'cam 0,0,12,7 | cam 0,7,12,7'],
    '3': [cams(3), 'cam 0,0,6,5 | cam 6,0,6,5 | cam 0,5,6,5', 'cam 0,0,6,5 | cam 6,0,6,5 | cam 0,5,6,5', 'cam 0,0,12,7 | cam 0,7,12,7 | cam 0,14,12,7'],
    'share+3': [[share, ...cams(3)], 'screen 0,0,12,7 | cam 0,7,6,5 | cam 6,7,6,5 | cam 0,12,6,5', 'screen 0,0,6,5 | cam 6,0,6,5 | cam 0,5,6,5 | cam 6,5,6,5', 'screen 0,0,12,7 | cam 0,7,4,4 | cam 4,7,4,4 | cam 8,7,4,4'],
  }
  for (const [name, [tiles, auto, even, focus]] of Object.entries(expected)) {
    assert.equal(flat(presetCells(tiles, 'Auto', {}, true)), auto, `Auto ${name}`)
    assert.equal(flat(presetCells(tiles, 'Even', {}, true)), even, `Even ${name}`)
    assert.equal(flat(presetCells(tiles, 'Focus', {}, true)), focus, `Focus ${name}`)
  }
})
