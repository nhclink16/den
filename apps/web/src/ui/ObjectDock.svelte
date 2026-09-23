<script lang="ts">
  import { objects } from '../lib/objects.svelte'
  import { objectKind } from '../plugins'
  import Icon from './Icon.svelte'
  let height = $state(0)
  let dock: HTMLDivElement
  const object = $derived(objects.active!)
  const View = $derived(objectKind(object.kind)?.view)
  $effect(() => { const saved = Number(localStorage.getItem(`den.object.height.${object.id}`)); height = Number.isFinite(saved) ? saved : 0 })
  function resize(e: PointerEvent) { e.preventDefault(); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId) }
  function move(e: PointerEvent) { if ((e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) change(e.clientY - dock.getBoundingClientRect().top) }
  function change(value: number) { height = Math.max(260, Math.min(value, (dock.parentElement?.clientHeight || 800) - 160)); localStorage.setItem(`den.object.height.${object.id}`, String(height)) }
  function close() { objects.active = null; objects.expanded = false }
  function key(e: KeyboardEvent) { if (e.key === 'Escape' && !e.defaultPrevented) { e.stopPropagation(); close() } }
</script>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="object-dock" class:expanded={objects.expanded} style:height={objects.expanded ? undefined : height ? `${height}px` : '45%'} bind:this={dock} onkeydown={key} role="region" aria-label={`${object.kind} ${object.name}`} data-testid={`${object.kind}-dock`}>
  {#key object.id}{#if View}<View {object} />{/if}{/key}
  <div class="controls">
    <button aria-label={objects.expanded ? `Collapse ${object.kind}` : `Expand ${object.kind}`} title={objects.expanded ? `Collapse ${object.kind}` : `Expand ${object.kind}`} onclick={() => objects.expanded = !objects.expanded}><Icon name={objects.expanded ? 'collapse' : 'expand'} size={16} /></button>
    <button aria-label={`Close ${object.kind}`} title={`Close ${object.kind}`} onclick={close}><Icon name="x" size={16} /></button>
  </div>
  {#if !objects.expanded}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
  <div class="resize" role="separator" aria-label={`${object.kind} height`} aria-orientation="horizontal" aria-valuenow={height || 360} tabindex="0" onpointerdown={resize} onpointermove={move} onkeydown={(e) => { if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); change(dock.clientHeight + (e.key === 'ArrowUp' ? -20 : 20)) } }}></div>{/if}
</div>
<style>
  .object-dock { position: relative; min-height: 260px; flex: none; border-bottom: 1px solid var(--line); isolation: isolate; }
  .object-dock.expanded { flex: 1; min-height: 0; }
  .controls { position: absolute; z-index: 400; top: 8px; right: 8px; display: flex; gap: 4px; border-radius: var(--r-pill, 999px); padding: 4px; background: var(--overlay); }
  .controls button { width: 28px; height: 28px; display: grid; place-items: center; border-radius: 50%; }
  .controls button:hover { background: var(--bg-3); }
  .resize { position: absolute; z-index: 400; bottom: 0; height: 6px; width: 100%; cursor: ns-resize; touch-action: none; background: var(--line); }
  .resize:hover, .resize:focus-visible { background: var(--ink-3); }
</style>
