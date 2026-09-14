import { test } from 'node:test'
import assert from 'node:assert/strict'
import { bounds, overlaps, pack, participantSlots, presetCells, type TileSpec } from '../apps/web/src/lib/call-layout.ts'
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
