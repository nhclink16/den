<script lang="ts">
  import { tick } from 'svelte'
  import { objectKind } from '../plugins'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallGrid from './CallGrid.svelte'
  import CallTile from './CallTile.svelte'
  import CallControls from './CallControls.svelte'
  import CallJam from './CallJam.svelte'
  import CallPill from './CallPill.svelte'
  import Icon from './Icon.svelte'
  // `narrow` (a phone) shows the call as a pill while you chat instead of a strip
  // of cameras. `back` names where collapsing the open call returns to, and
  // `titled` gives it its own title where no room header names the call.
  let { narrow = false, back = '', titled = false }: { narrow?: boolean; back?: string; titled?: boolean } = $props()
  const strip = $derived(!call.expanded && !narrow)
  // Opening or closing the call swaps the button you pressed for another view. When
  // that drops keyboard focus, hand it to the matching control: Back once open,
  // the opener once closed. Not on a touch screen: there's no keyboard focus to keep,
  // and opening a view shouldn't light up a button in it.
  let wasExpanded = call.expanded
  $effect(() => {
    const now = call.expanded
    if (now !== wasExpanded) void tick().then(() => {
      if (document.documentElement.dataset.input === 'touch') return
      if (document.activeElement && document.activeElement !== document.body) return
      dock?.querySelector<HTMLElement>(now ? '.collapse' : '.expand, .call-pill .open')?.focus()
    })
    wasExpanded = now
  })
  let dock: HTMLDivElement
  let height = $state(160)
  let maximum = $state(600)
  const heightKey = $derived(`den.call-height:${call.origin}|${call.channel?.id}`)
  $effect(() => {
    const key = heightKey
    try { const saved = Number(localStorage.getItem(key)); height = Number.isFinite(saved) && saved >= 100 ? saved : 160 } catch { height = 160 }
  })
  $effect(() => {
    const parent = dock?.parentElement
    if (!parent) return
    const observer = new ResizeObserver(() => { maximum = Math.max(100, parent.clientHeight - 160) })
    observer.observe(parent)
    return () => observer.disconnect()
  })
  const shownHeight = $derived(Math.max(100, Math.min(height, maximum)))
  // Everyone in the call stays on the strip: tiles shrink to fit, at 16:9, down to 160px wide, and only
  // past that does the strip scroll, with a fade at the edge for the people beyond it. Once tiles have
  // shrunk, the strip hugs them rather than leaving an empty band; it never grows past the height you chose.
  const Tile = $derived(objects.active ? objectKind(objects.active.kind)?.tile : undefined)
  const count = $derived((Tile ? 1 : 0) + call.participants.reduce((n, p) => n + 1 + p.screens.length, 0))
  let tilesWidth = $state(0), atEnd = $state(true), tiles = $state<HTMLDivElement>()
  const PAD_X = 28, PAD_Y = 26, GAP = 10
  const tileWidth = $derived(Math.min((shownHeight - PAD_Y) * 16 / 9, Math.max(160, (tilesWidth - PAD_X - (count - 1) * GAP) / Math.max(1, count))))
  const stripHeight = $derived(Math.min(shownHeight, Math.round(tileWidth * 9 / 16 + PAD_Y)))
  const over = $derived(tilesWidth > 0 && count * tileWidth + (count - 1) * GAP + PAD_X > tilesWidth + 1)
  const edge = () => { if (tiles) atEnd = tiles.scrollLeft + tiles.clientWidth >= tiles.scrollWidth - 2 }
  $effect(() => { void over; void tileWidth; edge() })
  // A mouse wheel scrolls the strip sideways when it overflows; the page around it doesn't move.
  $effect(() => {
    const el = tiles
    if (!el) return
    const wheel = (e: WheelEvent) => {
      if (!over || Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return
      e.preventDefault(); el.scrollLeft += e.deltaY
    }
    el.addEventListener('wheel', wheel, { passive: false })
    return () => el.removeEventListener('wheel', wheel)
  })
  function change(value: number) {
    height = Math.max(100, Math.min(value, maximum))
    try { localStorage.setItem(heightKey, String(height)) } catch { /* Resizing still works without storage. */ }
  }
  function resize(e: PointerEvent) { e.preventDefault(); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId) }
  function move(e: PointerEvent) { if ((e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) change(e.clientY - dock.getBoundingClientRect().top) }
  function resizeKey(e: KeyboardEvent) {
    if (['ArrowUp', 'ArrowDown', 'Home', 'End', 'Enter'].includes(e.key)) {
      e.preventDefault()
      change(e.key === 'Home' ? 100 : e.key === 'End' ? maximum : e.key === 'Enter' ? 160 : shownHeight + (e.key === 'ArrowUp' ? -20 : 20))
    }
  }
</script>

<div bind:this={dock} style:height={strip ? `${stripHeight}px` : undefined} class="call-view" class:expanded={call.expanded} class:strip class:docked={!call.expanded && narrow} data-testid={call.expanded ? 'call-grid' : strip ? 'call-strip' : 'call-docked'}>
  {#snippet deviceNote()}{#if call.otherDevices > 0 && !call.micOn && call.outputMuted}<p class="device-note" role="status">Mic and sound are off on this device. Your other device stays connected.</p>{/if}{/snippet}
  {#if call.expanded || !narrow}{@render deviceNote()}{/if}
  {#if call.expanded}
    <CallGrid {back} {titled} />
    <div class="toolbar"><CallControls variant="bar" /></div>
  {:else if narrow}
    <CallPill />
    {@render deviceNote()}
    <CallJam />
  {:else}
  <div class="tiles" class:fade={over && !atEnd} bind:this={tiles} bind:clientWidth={tilesWidth} onscroll={edge} style:--tile-w={`${tileWidth}px`}>
    {#if objects.active && Tile}<Tile object={objects.active} />{/if}
    {#each call.participants as participant (participant.id)}{#each participant.screens as share (share.name)}<CallTile {participant} {share} screen />{/each}{/each}
    {#each call.participants as participant (participant.id)}<CallTile {participant} />{/each}
  </div>
  <button class="expand" aria-label="Expand call" title="Expand call" onclick={() => (call.expanded = true)}><Icon name="expand" /></button>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="resize" role="separator" aria-label="Call strip height" aria-orientation="horizontal" aria-valuemin={100} aria-valuemax={maximum} aria-valuenow={shownHeight} aria-valuetext={`${shownHeight} pixels`} title="Drag to resize · Arrow keys to resize · Double-click or Enter to reset" tabindex="0" onpointerdown={resize} onpointermove={move} onkeydown={resizeKey} ondblclick={() => change(160)}></div>
  {/if}
</div>

<style>
  .call-view { flex: none; position: relative; min-height: 0; }
  /* A strip of cameras above the chat on a computer. Drag its bottom edge to resize. */
  .strip { height: 160px; display: flex; align-items: center; background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .resize { position: absolute; z-index: 4; bottom: 0; height: 8px; width: 100%; cursor: ns-resize; touch-action: none; }
  .resize::after { content: ''; display: block; width: 36px; height: 4px; margin: 2px auto; border-radius: 2px; background: var(--ink-3); opacity: .5; transition: opacity var(--t-fast); }
  .resize:focus-visible::after { opacity: 1; }
  @media (hover: hover) { .resize:hover::after { opacity: 1; } }
  /* [tiles][Expand]: Expand has a column of its own, so it never sits on a tile's menu. */
  .tiles { flex: 1; min-width: 0; height: 100%; display: flex; align-items: center; gap: 10px; padding: 12px 12px 14px 16px; overflow-x: auto; scrollbar-width: none; }
  .strip .tiles :global(.tile) { width: var(--tile-w); height: auto; aspect-ratio: 16/9; }
  /* A shrunken tile is too short for a full-size face and the name chip: the face halves and sits higher. */
  @container (max-width: 200px) { .strip .tiles :global(.tile > .face) { transform: scale(.5); margin-bottom: 24px; } }
  .tiles.fade { mask-image: linear-gradient(to left, transparent, #000 40px); }
  .tiles::-webkit-scrollbar { display: none; }
  .expand { z-index: 3; flex: none; margin-inline-end: 12px; width: 32px; height: 32px; display: grid; place-items: center; border-radius: var(--r-pill); color: var(--ink); background: color-mix(in srgb, var(--ink) 11%, transparent); }
  @media (hover: hover) { .expand:hover { background: color-mix(in srgb, var(--ink) 18%, transparent); } }
  /* The open call fills the room: a header row, the tiles, and a floating bar of
     round buttons. No backdrop of its own, so a chat wallpaper shows around the tiles. */
  .expanded { flex: 1; display: flex; flex-direction: column; container: callview / inline-size; }
  .toolbar { z-index: 3; position: absolute; bottom: 20px; left: 50%; transform: translateX(-50%); width: max-content; max-width: calc(100% - 16px); padding: 8px; border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(20px) saturate(1.2); box-shadow: var(--lift), 0 0 0 1px color-mix(in srgb, var(--line) 60%, transparent); }
  .toolbar :global(.controls) { justify-content: center; }
  /* Clear of the iPhone's home bar. */
  @media (max-width: 600px) { .toolbar { bottom: calc(12px + env(safe-area-inset-bottom, 0px)); } }
  .device-note { position: absolute; left: 12px; top: 4px; z-index: 5; margin: 0; padding: 4px 10px; max-width: calc(100% - 56px); border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(10px); font-size: var(--fs-xs); color: var(--ink-2); }
  .expanded .device-note { top: auto; bottom: 100px; left: 50%; transform: translateX(-50%); }
  .docked .device-note { position: static; margin: 6px 16px; }
</style>
