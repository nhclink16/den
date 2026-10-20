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
  // the opener once closed.
  let wasExpanded = call.expanded
  $effect(() => {
    const now = call.expanded
    if (now !== wasExpanded) void tick().then(() => {
      if (document.activeElement && document.activeElement !== document.body) return
      dock?.querySelector<HTMLElement>(now ? '.call-head .collapse' : '.expand, .call-pill .open')?.focus()
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

<div bind:this={dock} style:height={strip ? `${shownHeight}px` : undefined} class="call-view" class:expanded={call.expanded} class:strip class:docked={!call.expanded && narrow} data-testid={call.expanded ? 'call-grid' : strip ? 'call-strip' : 'call-docked'}>
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
  <div class="tiles">
    {#if objects.active}
      {@const Tile = objectKind(objects.active.kind)?.tile}
      {#if Tile}<Tile object={objects.active} />{/if}
    {/if}
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
  .strip { height: 160px; background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .resize { position: absolute; z-index: 4; bottom: 0; height: 8px; width: 100%; cursor: ns-resize; touch-action: none; }
  .resize::after { content: ''; display: block; width: 36px; height: 4px; margin: 2px auto; border-radius: 2px; background: var(--ink-3); opacity: .5; transition: opacity var(--t-fast); }
  .resize:focus-visible::after { opacity: 1; }
  @media (hover: hover) { .resize:hover::after { opacity: 1; } }
  .tiles { height: 100%; display: flex; align-items: center; gap: 10px; padding: 12px 56px 14px 16px; overflow-x: auto; scrollbar-width: none; }
  .strip .tiles :global(.tile) { height: 100%; width: auto; aspect-ratio: 16/9; }
  .tiles::-webkit-scrollbar { display: none; }
  .expand { z-index: 3; position: absolute; top: 12px; right: 12px; width: 32px; height: 32px; display: grid; place-items: center; border-radius: var(--r-pill); color: var(--ink); background: color-mix(in srgb, var(--ink) 11%, transparent); }
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
  .docked .device-note { position: static; margin: 0 16px 6px; }
</style>
