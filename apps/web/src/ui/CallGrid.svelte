<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import type { ObjectSummary } from '../lib/types'
  import { objects } from '../lib/objects.svelte'
  import { objectKind } from '../plugins'
  import { bounds, pack, participantSlots, presetCells, type Preset, type Cell, type TileSpec } from '../lib/call-layout'
  import { callLayouts } from '../lib/call-layout.svelte'
  import CallLayoutTile from './CallLayoutTile.svelte'
  type Tile = TileSpec & { participant?: CallParticipant; share?: Share; object?: ObjectSummary }
  let mobile = $state(window.innerWidth < 900), width = $state(0), height = $state(0)
  // Two different questions, and conflating them swapped people's layouts. How to
  // arrange tiles depends on the room the grid actually has, so it follows the
  // container. Which saved layout to load is a property of the window, so it follows
  // the window: hiding the member list makes the grid wider, and that must not count
  // as a rotation and hand you the other orientation's arrangement.
  let windowPortrait = $state(matchMedia('(orientation: portrait)').matches)
  const portrait = $derived(height > width && width > 0)
  const stacked = $derived(mobile || portrait)
  let scroll: HTMLDivElement, canvas: HTMLDivElement
  let preview = $state<Record<string, Cell> | null>(null), dragging = $state('')
  let cancelDrag: (() => void) | undefined
  // Orientation is read outside untrack so a real rotation reloads that shape's layout.
  $effect(() => { const id = call.channel?.id, shape = windowPortrait; if (id) untrack(() => callLayouts.open(id, mobile, shape)) })
  onMount(() => {
    const mq = matchMedia('(max-width: 899px)'), update = () => { mobile = mq.matches; cancelDrag?.() }
    const turn = matchMedia('(orientation: portrait)'), rotate = () => { windowPortrait = turn.matches; cancelDrag?.() }
    mq.addEventListener('change', update)
    turn.addEventListener('change', rotate)
    return () => { mq.removeEventListener('change', update); turn.removeEventListener('change', rotate); cancelDrag?.() }
  })
  const tiles = $derived.by<Tile[]>(() => {
    const slots = participantSlots(call.participants.map(p => p.id), callLayouts.value.tiles)
    const out: Tile[] = []
    for (const participant of call.participants) for (const share of participant.screens) out.push({ key: `${slots[participant.id]}:screen:${share.name}`, kind: 'screen', participant, share })
    const object = objects.active
    if (object && objectKind(object.kind)?.tile && (object.kind === 'canvas' || object.kind === 'terminal')) out.push({ key: `${object.created_by}:${object.kind}:${object.id}`, kind: object.kind, object })
    for (const participant of call.participants) out.push({ key: `${slots[participant.id]}:cam`, kind: 'cam', participant })
    return out
  })
  const preset = $derived(mobile && callLayouts.value.preset === 'Custom' ? 'Focus' : callLayouts.value.preset)
  const cells = $derived(preview || (preset === 'Custom' ? pack(tiles, callLayouts.value.tiles) : presetCells(tiles, preset, callLayouts.value.tiles, stacked)))
  const rows = $derived(Math.max(1, ...Object.values(cells).map(c => c.row + c.h)))
  const stepX = $derived((width + 8) / 12)
  // Keep preset rows within the viewport when practical; custom layouts can scroll.
  // The ceiling is a ratio of the column width so a tile keeps a sane shape. Portrait
  // gets a taller ceiling: bounding rows by width alone left most of a tall screen
  // empty, which is what a vertical monitor looked like before. Anything still left
  // over is split above and below by the canvas margins rather than all falling below.
  const stepY = $derived(mobile ? Math.max(24, stepX) : Math.max(24, Math.min(stepX * (portrait ? 1.3 : .8), (height - 8) / rows)))
  function choose(value: Preset) { cancelDrag?.(); callLayouts.save(value, presetCells(tiles, value, callLayouts.value.tiles, mobile)) }
  function reset() { cancelDrag?.(); callLayouts.reset() }
  function pin(key: string) {
    // Mobile presents Custom as Focus; pinning must not save its display geometry
    // over the desktop arrangement. Only the pin changes in that case.
    const saved = callLayouts.value
    const original = mobile && saved.preset === 'Custom' ? saved.tiles[key] || cells[key]! : cells[key]!
    callLayouts.save(saved.preset, { ...(mobile && saved.preset === 'Custom' ? {} : cells), [key]: { ...original, pinned: !original.pinned } })
  }
  function move(key: string, dx: number, dy: number, resize: boolean) {
    const old = cells[key]!
    const next = bounds({ ...old, ...(resize ? { w: old.w + dx, h: old.h + dy } : { col: old.col + dx, row: old.row + dy }) })
    callLayouts.save('Custom', pack(tiles, { ...cells, [key]: next }, key))
  }
  function drag(key: string, e: PointerEvent, resize: boolean) {
    if (mobile || e.button !== 0) return
    e.preventDefault(); cancelDrag?.()
    const target = e.currentTarget as HTMLElement
    const root = target.closest<HTMLElement>('.layout-tile')!; root.focus()
    const start = { x: e.clientX, y: e.clientY, scroll: scroll.scrollTop, stepY, cells: { ...cells }, tile: { ...cells[key]! } }
    target.setPointerCapture(e.pointerId)
    let moved = false
    function update(event: PointerEvent) {
      if (event.pointerId !== e.pointerId) return
      if (!moved && Math.hypot(event.clientX - start.x, event.clientY - start.y) < 4) return
      moved = true; dragging = key
      const box = scroll.getBoundingClientRect()
      if (event.clientY > box.bottom - 30) scroll.scrollTop += 14
      if (event.clientY < box.top + 30) scroll.scrollTop -= 14
      const dx = Math.round((event.clientX - start.x) / stepX), dy = Math.round((event.clientY - start.y + scroll.scrollTop - start.scroll) / start.stepY)
      const next = bounds({ ...start.tile, ...(resize ? { w: start.tile.w + dx, h: start.tile.h + dy } : { col: start.tile.col + dx, row: start.tile.row + dy }) })
      preview = pack(tiles, { ...start.cells, [key]: next }, key)
    }
    function finish(event?: PointerEvent) {
      if (event && event.pointerId !== e.pointerId) return
      if (event?.type === 'pointerup' && moved && preview) callLayouts.save('Custom', preview)
      target.removeEventListener('pointermove', update); target.removeEventListener('pointerup', finish); target.removeEventListener('pointercancel', finish); target.removeEventListener('lostpointercapture', finish)
      if (target.hasPointerCapture(e.pointerId)) target.releasePointerCapture(e.pointerId)
      preview = null; dragging = ''; cancelDrag = undefined
    }
    cancelDrag = () => finish()
    target.addEventListener('pointermove', update); target.addEventListener('pointerup', finish); target.addEventListener('pointercancel', finish); target.addEventListener('lostpointercapture', finish)
  }
</script>

<div class="presets" role="group" aria-label="Call layout">
  {#each ['Auto', 'Even', 'Focus'] as p}<button aria-pressed={preset === p} onclick={() => choose(p as Preset)}>{p}</button>{/each}
  {#if preset === 'Custom'}<span>Custom</span>{/if}
</div>
<div class="grid-scroll" bind:this={scroll} bind:clientHeight={height}>
  <div class="grid-canvas" class:dragging={!!dragging} bind:this={canvas} bind:clientWidth={width}
    style={`--step-x:${stepX}px;--step-y:${stepY}px;height:${rows * stepY}px`}>
    {#each tiles as tile (tile.key)}
      <CallLayoutTile tileKey={tile.key} cell={cells[tile.key]!} participant={tile.participant} share={tile.share} object={tile.object} {mobile}
        onpin={() => pin(tile.key)} onmove={(dx, dy, resize) => move(tile.key, dx, dy, resize)} onreset={reset} ondrag={(e, resize) => drag(tile.key, e, resize)} />
    {/each}
    {#if dragging && cells[dragging]}
      {@const c = cells[dragging]!}
      <div class="drop-target" style={`left:${c.col * stepX}px;top:${c.row * stepY}px;width:${c.w * stepX - 8}px;height:${c.h * stepY - 8}px`}></div>
    {/if}
  </div>
</div>

<style>
  .presets { position: absolute; top: 10px; left: 16px; display: flex; align-items: center; border: 1px solid var(--line); border-radius: 6px; z-index: 2; overflow: hidden; background: var(--bg-2); }
  .presets button, .presets span { padding: 5px 10px; font: 11px var(--mono); color: var(--ink-3); }
  .presets button[aria-pressed="true"] { color: var(--ink); background: var(--bg-3); }
  .presets button:hover { color: var(--ink); }
  .grid-scroll { position: absolute; inset: 48px 16px 82px; overflow: auto; scrollbar-width: thin; display: flex; }
  /* Auto margins centre the rows when they are shorter than the viewport and
     collapse to nothing when they overflow, which keeps the top row reachable. */
  .grid-canvas { position: relative; width: 100%; flex: none; margin-block: auto; }
  .dragging { background-image: linear-gradient(to right, var(--grid-guide) 1px, transparent 1px), linear-gradient(to bottom, var(--grid-guide) 1px, transparent 1px); background-size: var(--step-x) var(--step-y); }
  .dragging :global(.layout-tile) { opacity: .8; }
  .drop-target { position: absolute; pointer-events: none; border: 2px solid var(--lamp); border-radius: var(--r-lg); background: var(--grid-guide); }
  @media (max-width: 899px) { .grid-scroll { left: 10px; right: 10px; } .presets { left: 10px; } }
</style>
