<script lang="ts">
  import { objectKind } from '../plugins'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallGrid from './CallGrid.svelte'
  import CallTile from './CallTile.svelte'
  import CallControls from './CallControls.svelte'
  import Icon from './Icon.svelte'
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

<div bind:this={dock} style:height={call.expanded ? undefined : `${shownHeight}px`} class="call-view" class:expanded={call.expanded} data-testid={call.expanded ? 'call-grid' : 'call-strip'}>
  {#if call.otherDevices > 0 && !call.micOn && call.outputMuted}<p class="device-note" role="status">Mic and sound are off on this device. Your other device stays connected.</p>{/if}
  {#if call.expanded}<CallGrid />{:else}
  <div class="tiles">
    {#if objects.active}
      {@const Tile = objectKind(objects.active.kind)?.tile}
      {#if Tile}<Tile object={objects.active} />{/if}
    {/if}
    {#each call.participants as participant (participant.id)}{#each participant.screens as share (share.name)}<CallTile {participant} {share} screen />{/each}{/each}
    {#each call.participants as participant (participant.id)}<CallTile {participant} />{/each}
  </div>
  {/if}
  <button class="expand" aria-label={call.expanded ? 'Collapse call' : 'Expand call'} title={call.expanded ? 'Collapse call' : 'Expand call'} onclick={() => (call.expanded = !call.expanded)}><Icon name={call.expanded ? 'collapse' : 'expand'} /></button>
  {#if !call.expanded}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div class="resize" role="separator" aria-label="Call strip height" aria-orientation="horizontal" aria-valuemin={100} aria-valuemax={maximum} aria-valuenow={shownHeight} aria-valuetext={`${shownHeight} pixels`} title="Drag to resize · Arrow keys to resize · Double-click or Enter to reset" tabindex="0" onpointerdown={resize} onpointermove={move} onkeydown={resizeKey} ondblclick={() => change(160)}></div>
  {/if}
  {#if call.expanded}<div class="toolbar"><CallControls large /></div>{/if}
</div>

<style>
  .resize { position: absolute; z-index: 4; bottom: 0; height: 8px; width: 100%; cursor: ns-resize; touch-action: none; background: var(--line); }
  .resize::after { content: ''; display: block; width: 36px; height: 2px; margin: 3px auto; border-radius: 2px; background: var(--ink-3); }
  .resize:hover, .resize:focus-visible { background: var(--bg-3); }
  .expanded .device-note { top: 40px; }
  .device-note { position: absolute; left: 12px; top: 4px; z-index: 1; margin: 0; padding: 4px 8px; max-width: calc(100% - 56px); border-radius: var(--r); background: var(--bg); font-size: 12px; color: var(--ink-2); }
  .call-view { height: 160px; flex: none; position: relative; min-height: 0; background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .tiles { height: 100%; display: flex; align-items: center; gap: 10px; padding: 12px; overflow-x: auto; scrollbar-width: none; }
  .call-view:not(.expanded) .tiles :global(.tile) { height: 100%; width: auto; aspect-ratio: 16/9; }
  .tiles::-webkit-scrollbar { display: none; }
  .expand { z-index: 3; position: absolute; top: 6px; right: 6px; width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--line); border-radius: var(--r); color: var(--ink-2); background: var(--bg-2); }
  .expand:hover { color: var(--ink); }
  .expanded { flex: 1; height: auto; }
  .toolbar { z-index: 3; position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%); padding: 8px; border-radius: var(--r-lg); border: 1px solid var(--line); background: var(--bg-2); box-shadow: 0 8px 24px var(--shadow); max-width: calc(100% - 16px); }
</style>
