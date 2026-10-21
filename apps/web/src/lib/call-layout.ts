export type Preset = 'Auto' | 'Even' | 'Focus' | 'Custom'
export type Cell = { col: number; row: number; w: number; h: number; pinned: boolean }
export type Layout = { preset: Preset; tiles: Record<string, Cell> }
export type TileSpec = { key: string; kind: 'cam' | 'screen' | 'canvas' | 'terminal' }
export const columns = 12
// A layout authored for a wide window does not survive a rotation: its columns are
// relative, so a tile that was a comfortable half of a landscape window becomes a
// sliver of a portrait one. Keep one per orientation instead of reflowing someone's
// arrangement out from under them. Landscape keeps the original key, so nobody's
// existing layout moves.
export const storageKey = (room: string, portrait = false) =>
  `den.call-layout:${room}${portrait ? ':portrait' : ''}`
const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, Math.round(n)))
export function bounds(cell: Cell): Cell {
  const w = clamp(cell.w, 2, columns), h = clamp(cell.h, 2, 24)
  return { col: clamp(cell.col, 0, columns - w), row: clamp(cell.row, 0, 200), w, h, pinned: !!cell.pinned }
}
export function overlaps(a: Cell, b: Cell) {
  return a.col < b.col + b.w && a.col + a.w > b.col && a.row < b.row + b.h && a.row + a.h > b.row
}
export function readLayout(room: string, mobile: boolean, portrait = false): Layout {
  const empty: Layout = { preset: mobile ? 'Focus' : 'Auto', tiles: {} }
  try {
    const raw = JSON.parse(localStorage.getItem(storageKey(room, portrait)) || 'null')
    if (!raw || !['Auto', 'Even', 'Focus', 'Custom'].includes(raw.preset) || !raw.tiles || typeof raw.tiles !== 'object') return empty
    const tiles: Record<string, Cell> = {}
    for (const [key, value] of Object.entries(raw.tiles).slice(-200)) {
      const v = value as Cell
      if (v && [v.col, v.row, v.w, v.h].every(Number.isFinite)) tiles[key] = bounds(v)
    }
    return { preset: raw.preset, tiles }
  } catch { return empty }
}
// Reuse vacant participant slots after LiveKit rotates connection identities. Reserve
// exact matches first so two devices on one account can never claim the same slot.
export function participantSlots(ids: string[], saved: Record<string, Cell>): Record<string, string> {
  const slots = [...new Set(Object.keys(saved).filter(k => k.endsWith(':cam')).map(k => k.slice(0, k.lastIndexOf(':'))))]
  const result: Record<string, string> = {}, used = new Set<string>()
  for (const id of ids) if (slots.includes(id)) { result[id] = id; used.add(id) }
  for (const id of [...ids].sort()) if (!result[id]) {
    const slot = slots.find(s => s.split(':')[0] === id.split(':')[0] && !used.has(s)) || id
    result[id] = slot; used.add(slot)
  }
  return result
}
// A landscape grid's row is .8 of a column (CallGrid), so this many rows keeps a
// tile `w` columns wide close to 16:9.
const tall = (w: number) => Math.max(2, Math.round(w * .7))
// Rows that `n` tiles `w` columns wide take, wrapping as many across as fit.
const rowsFor = (n: number, w: number) => n && Math.ceil(n / Math.min(n, Math.floor(columns / w))) * tall(w)
// `stacked` means the container is taller than it is wide: a phone, or a desktop
// monitor rotated into portrait. Both want fewer tiles across and taller rows.
// `fit` is how many full-height rows a landscape stage has room for; tiles are sized
// to stay within it, so a lone camera is one large 16:9 tile, not a full-width strip.
// Nine is a 16:10 window, for callers that don't know the stage.
export function presetCells(tiles: TileSpec[], preset: Preset, saved: Record<string, Cell>, stacked = false, fit = 9): Record<string, Cell> {
  const cells: Record<string, Cell> = {}
  let row = 0
  function band(items: TileSpec[], across: number, h: number) {
    if (!items.length) return
    across = Math.min(across, items.length)
    const w = Math.floor(columns / across)
    for (const [i, t] of items.entries()) cells[t.key] = { col: i % across * w, row: row + Math.floor(i / across) * h, w, h, pinned: saved[t.key]?.pinned || false }
    row += Math.ceil(items.length / across) * h
  }
  // Tiles `w` columns wide, each line centred. A half column is fine here: `bounds`
  // rounds it when a cell is dragged, packed or loaded.
  function grid(items: TileSpec[], w: number) {
    const across = Math.min(items.length, Math.floor(columns / w)), h = tall(w)
    for (const [i, t] of items.entries()) {
      const line = Math.floor(i / across), count = Math.min(across, items.length - line * across)
      cells[t.key] = { col: (columns - count * w) / 2 + i % across * w, row: row + line * h, w, h, pinned: saved[t.key]?.pinned || false }
    }
    row += rowsFor(items.length, w)
  }
  // The widest tiles that fit in `space` rows. When even three columns (four across)
  // don't fit, CallGrid squeezes the rows, as it always has.
  function fill(items: TileSpec[], space: number) {
    let w = columns
    while (w > 3 && rowsFor(items.length, w) > space) w--
    grid(items, w)
  }
  if (!stacked) {
    // Focus keeps its thumbnails smaller than Auto's cameras under a share.
    const pinned = tiles.filter(t => saved[t.key]?.pinned), shared = tiles.filter(t => t.kind !== 'cam')
    const large = preset === 'Even' ? tiles : preset === 'Focus' ? (pinned.length ? pinned : shared.length ? shared : tiles) : shared
    const small = tiles.filter(t => !large.includes(t)), w = preset === 'Focus' ? 2 : 3
    if (!large.length) fill(small, fit)
    else { fill(large, fit - rowsFor(small.length, w)); grid(small, w) }
    return cells
  }
  if (preset === 'Even') band(tiles, 2, 5)
  else if (preset === 'Focus') {
    let large = tiles.filter(t => saved[t.key]?.pinned)
    if (!large.length) large = tiles.filter(t => t.kind !== 'cam')
    if (!large.length) large = tiles
    band(large, 1, 7)
    band(tiles.filter(t => !large.includes(t)), 3, 4)
  } else {
    band(tiles.filter(t => t.kind === 'screen'), 1, 7)
    band(tiles.filter(t => t.kind === 'canvas' || t.kind === 'terminal'), 1, 7)
    // Two cams side by side across a narrow axis leaves each tile far taller than
    // the 16:9 inside it. Stacked and full width they stay close to their own shape.
    const cams = tiles.filter(t => t.kind === 'cam')
    band(cams, cams.length <= 2 ? 1 : 2, 5)
  }
  return cells
}
// Active cells alone occupy space. A moved/resized tile wins its drop rectangle;
// colliding neighbours shift to the first available cell, without hiding anything.
export function pack(tiles: TileSpec[], saved: Record<string, Cell>, priority?: string): Record<string, Cell> {
  const placed: Record<string, Cell> = {}, defaults = presetCells(tiles, 'Auto', saved)
  const ordered = [...tiles].sort((a, b) => a.key === priority ? -1 : b.key === priority ? 1 : 0)
  for (const tile of ordered) {
    const desired = bounds(saved[tile.key] || defaults[tile.key])
    const free = (c: Cell) => Object.values(placed).every(p => !overlaps(c, p))
    if (!free(desired)) {
      let found = false
      for (let row = 0; !found; row++) for (let col = 0; col <= columns - desired.w; col++) {
        const next = { ...desired, row, col }
        if (free(next)) { desired.row = row; desired.col = col; found = true; break }
      }
    }
    placed[tile.key] = desired
  }
  return placed
}
