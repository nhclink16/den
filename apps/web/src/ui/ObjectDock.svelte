<script lang="ts">
  import { objects } from '../lib/objects.svelte'
  import { objectKind } from '../plugins'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  let height = $state(0)
  let dock: HTMLDivElement
  const object = $derived(objects.active!)
  const View = $derived(objectKind(object.kind)?.view)
  // Who has it open: the same faces and live dot as its card.
  const people = $derived(objects.presence[object.id] || [])
  $effect(() => { const saved = Number(localStorage.getItem(`den.object.height.${object.id}`)); height = Number.isFinite(saved) ? saved : 0 })
  function resize(e: PointerEvent) { e.preventDefault(); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId) }
  function move(e: PointerEvent) { if ((e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) change(e.clientY - dock.getBoundingClientRect().top) }
  function change(value: number) { height = Math.max(260, Math.min(value, (dock.parentElement?.clientHeight || 800) - 160)); localStorage.setItem(`den.object.height.${object.id}`, String(height)) }
  function close() { objects.active = null; objects.expanded = false }
  function key(e: KeyboardEvent) { if (e.key === 'Escape' && !e.defaultPrevented) { e.stopPropagation(); close() } }
</script>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="object-dock" class:expanded={objects.expanded} style:height={objects.expanded || !height ? undefined : `${height}px`} bind:this={dock} onkeydown={key} role="region" aria-label={`${object.kind} ${object.name}`} data-testid={`${object.kind}-dock`}>
  <!-- A row of its own: which object this is and who has it open, then Den's buttons, so they never
       float over the object's own toolbars. Expanded, it is what still names the object. -->
  <div class="head">
    {#if object.kind === 'canvas' || object.kind === 'terminal'}<Icon name={object.kind} />{/if}
    <span class="name">{object.name || `Untitled ${object.kind}`}</span>
    {#if people.length}<span class="people" aria-label={`${people.length} on this ${object.kind}`}><span class="live"></span>{#each people as userId (userId)}<Avatar {userId} size={20} />{/each}</span>{/if}
    <button class="icon-btn grow-before" aria-label={objects.expanded ? `Collapse ${object.kind}` : `Expand ${object.kind}`} title={objects.expanded ? `Collapse ${object.kind}` : `Expand ${object.kind}`} onclick={() => objects.expanded = !objects.expanded}><Icon name={objects.expanded ? 'collapse' : 'expand'} size={16} /></button>
    <button class="icon-btn" aria-label={`Close ${object.kind}`} title={`Close ${object.kind}`} onclick={close}><Icon name="x" size={16} /></button>
  </div>
  <div class="body">{#key object.id}{#if View}<View {object} />{/if}{/key}</div>
  {#if !objects.expanded}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
  <div class="resize" role="separator" aria-label={`${object.kind} height`} aria-orientation="horizontal" aria-valuenow={height || 360} tabindex="0" onpointerdown={resize} onpointermove={move} onkeydown={(e) => { if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); change(dock.clientHeight + (e.key === 'ArrowUp' ? -20 : 20)) } }}></div>{/if}
</div>
<style>
  /* The header adds its height to the dock rather than taking it from the object: a canvas fits its
     drawing to the area it is given, so it keeps today's size and position. */
  .object-dock { --head: 44px; position: relative; display: flex; flex-direction: column; height: calc(45% + var(--head) + 1px); min-height: 260px; flex: none; border-bottom: 1px solid var(--line); isolation: isolate; }
  @media (max-width: 600px) { .object-dock { --head: 48px; } }
  .object-dock.expanded { flex: 1; height: auto; min-height: 0; }
  .head { display: flex; align-items: center; gap: var(--s2); flex: none; height: var(--head); padding-inline: var(--s4) var(--s1); background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .head > :global(svg) { flex: none; color: var(--ink-2); }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-sm); font-weight: 600; color: var(--ink); }
  .grow-before { margin-inline-start: auto; }
  /* The avatar stack, drawn as on the canvas and terminal cards. */
  .people { display: flex; flex: none; align-items: center; margin-inline-start: var(--s1); padding-inline-start: 6px; }
  .people :global(.avatar) { margin-inline-start: -6px; box-shadow: 0 0 0 2px var(--bg-2); }
  .people :global(.dot) { display: none; }
  .live { width: 7px; height: 7px; border-radius: 50%; background: var(--lamp); box-shadow: 0 0 8px var(--lamp); margin-inline-end: 10px; }
  .body { position: relative; flex: 1; min-height: 0; isolation: isolate; }
  .resize { position: absolute; z-index: 400; bottom: 0; height: 6px; width: 100%; cursor: ns-resize; touch-action: none; background: var(--line); }
  .resize:focus-visible { background: var(--ink-3); }
  @media (hover: hover) { .resize:hover { background: var(--ink-3); } }
</style>
