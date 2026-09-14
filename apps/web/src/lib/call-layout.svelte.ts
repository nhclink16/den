import { readLayout, storageKey, type Layout, type Preset, type Cell } from './call-layout'

class CallLayouts {
  room = $state('')
  value = $state<Layout>({ preset: 'Auto', tiles: {} })
  open(room: string, mobile: boolean) {
    if (room === this.room) return
    this.room = room; this.value = readLayout(room, mobile)
  }
  save(preset: Preset, cells: Record<string, Cell>) {
    // Dormant tiles are remembered but never participate in packing.
    const tiles = Object.fromEntries(Object.entries({ ...this.value.tiles, ...cells }).slice(-200))
    this.value = { preset, tiles }
    try { localStorage.setItem(storageKey(this.room), JSON.stringify(this.value)) } catch { /* Layout still works without storage. */ }
  }
  reset(room = this.room, mobile = window.innerWidth < 900) {
    try { localStorage.removeItem(storageKey(room)) } catch { /* Storage may be unavailable. */ }
    if (this.room === room) this.value = { preset: mobile ? 'Focus' : 'Auto', tiles: {} }
  }
}
export const callLayouts = new CallLayouts()
