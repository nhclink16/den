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
  assert.equal(wide['a:cam'].w, 6)
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
