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
  let scroll: HTMLDivElement, canvas: HTMLDivElement
  let preview = $state<Record<string, Cell> | null>(null), dragging = $state('')
  let cancelDrag: (() => void) | undefined
  $effect(() => { const id = call.channel?.id; if (id) untrack(() => callLayouts.open(id, mobile)) })
  onMount(() => {
    const mq = matchMedia('(max-width: 899px)'), update = () => { mobile = mq.matches; cancelDrag?.() }
    mq.addEventListener('change', update)
    return () => { mq.removeEventListener('change', update); cancelDrag?.() }
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
  const cells = $derived(preview || (preset === 'Custom' ? pack(tiles, callLayouts.value.tiles) : presetCells(tiles, preset, callLayouts.value.tiles, mobile)))
  const rows = $derived(Math.max(1, ...Object.values(cells).map(c => c.row + c.h)))
  const stepX = $derived((width + 8) / 12)
  // Keep preset rows within the viewport when practical; custom layouts can scroll.
  const stepY = $derived(mobile ? Math.max(24, stepX) : Math.max(24, Math.min(stepX * .8, (height - 8) / rows)))
  function choose(value: Preset) { cancelDrag?.(); callLayouts.save(value, presetCells(tiles, value, callLayouts.value.tiles, mobile)) }
  function reset() { cancelDrag?.(); callLayouts.reset() }
  function pin(key: string) { callLayouts.save(callLayouts.value.preset, { ...cells, [key]: { ...cells[key]!, pinned: !cells[key]!.pinned } }) }
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
    style={`--step-x:${stepX}px;--step-y:${stepY}px;height:${Math.max(height, rows * stepY)}px`}>
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
  .grid-scroll { position: absolute; inset: 48px 16px 82px; overflow: auto; scrollbar-width: thin; }
  .grid-canvas { position: relative; width: 100%; }
  .dragging { background-image: linear-gradient(to right, rgba(255,255,255,.06) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,.06) 1px, transparent 1px); background-size: var(--step-x) var(--step-y); }
  .dragging :global(.layout-tile) { opacity: .8; }
  .drop-target { position: absolute; pointer-events: none; border: 2px solid var(--lamp); border-radius: var(--r-lg); background: rgba(255,255,255,.06); }
  @media (max-width: 899px) { .grid-scroll { left: 10px; right: 10px; } .presets { left: 10px; } }
</style>
