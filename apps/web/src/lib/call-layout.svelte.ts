import { readLayout, storageKey, type Layout, type Preset, type Cell } from './call-layout'

class CallLayouts {
  room = $state('')
  // Layouts are per room and per orientation, so rotating a monitor loads the one
  // built for that shape rather than squeezing a landscape arrangement sideways.
  portrait = $state(false)
  value = $state<Layout>({ preset: 'Auto', tiles: {} })
  open(room: string, mobile: boolean, portrait = false) {
    if (room === this.room && portrait === this.portrait) return
    this.room = room; this.portrait = portrait; this.value = readLayout(room, mobile, portrait)
  }
  save(preset: Preset, cells: Record<string, Cell>) {
    // Dormant tiles are remembered but never participate in packing.
    const tiles = Object.fromEntries(Object.entries({ ...this.value.tiles, ...cells }).slice(-200))
    this.value = { preset, tiles }
    try { localStorage.setItem(storageKey(this.room, this.portrait), JSON.stringify(this.value)) } catch { /* Layout still works without storage. */ }
  }
  reset(room = this.room, mobile = window.innerWidth < 900) {
    // Reset means reset: clear both orientations, not just the one on screen.
    for (const portrait of [false, true])
      try { localStorage.removeItem(storageKey(room, portrait)) } catch { /* Storage may be unavailable. */ }
    if (this.room === room) this.value = { preset: mobile ? 'Focus' : 'Auto', tiles: {} }
  }
}
export const callLayouts = new CallLayouts()
