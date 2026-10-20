<script lang="ts">
  import { mount, unmount, onMount } from 'svelte'
  import { popTile } from '../lib/call-popout'
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import type { ObjectSummary } from '../lib/types'
  import type { Cell } from '../lib/call-layout'
  import CallTileContent from './CallTileContent.svelte'
  import Icon from './Icon.svelte'
  let { tileKey, cell, participant, share, object, mobile, onpin, onmove, onreset, ondrag }:
    { tileKey: string; cell: Cell; participant?: CallParticipant; share?: Share; object?: ObjectSummary; mobile: boolean;
      onpin: () => void; onmove: (dx: number, dy: number, resize: boolean) => void; onreset: () => void; ondrag: (e: PointerEvent, resize: boolean) => void } = $props()
  let host: HTMLDivElement, content: HTMLDivElement
  let popped = $state(false), opening = false, alive = false
  let closePop: (() => void) | undefined
  const contentProps = $state<{ participant?: CallParticipant; share?: Share; object?: ObjectSummary }>({})
  $effect(() => { contentProps.participant = participant; contentProps.share = share; contentProps.object = object })
  onMount(() => {
    alive = true; content = document.createElement('div'); content.className = 'call-content'; host.append(content)
    const component = mount(CallTileContent, { target: content, props: contentProps })
    return () => { alive = false; closePop?.(); void unmount(component); content.remove() }
  })
  async function pop() {
    if (mobile || opening) return
    if (popped) { closePop?.(); return }
    opening = true
    try {
      const close = await popTile(content, host, () => { popped = false; closePop = undefined })
      if (!alive) { close(); return }
      closePop = close; popped = true
    } catch (err) { call.report(err) } finally { opening = false }
  }
  function key(e: KeyboardEvent) {
    if (e.target !== e.currentTarget || e.ctrlKey || e.metaKey || e.altKey) return
    const k = e.key.toLowerCase()
    if (['arrowleft', 'arrowright', 'arrowup', 'arrowdown', 'p', 'o', 'r', 'escape'].includes(k)) {
      e.preventDefault(); e.stopPropagation()
      if (k === 'p') onpin()
      else if (k === 'o') void pop()
      else if (k === 'r') onreset()
      else if (k === 'escape') (e.currentTarget as HTMLElement).blur()
      else if (!mobile) onmove(k === 'arrowleft' ? -1 : k === 'arrowright' ? 1 : 0, k === 'arrowup' ? -1 : k === 'arrowdown' ? 1 : 0, e.shiftKey)
    }
  }
  function pointer(e: PointerEvent) {
    if (mobile || popped || e.button !== 0 || (e.target as HTMLElement).closest('button,summary,details,input,textarea,canvas,[contenteditable="true"],[data-terminal-focus]')) return
    ondrag(e, false)
  }
</script>

<!-- A tile is a group with nested controls, and its own documented move/resize keyboard interaction. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="layout-tile" class:pinned={cell.pinned} class:mobile class:plugin={!!object} data-tile-key={tileKey}
  data-col={cell.col} data-row={cell.row} data-w={cell.w} data-h={cell.h} data-kind={object?.kind || (share ? 'screen' : 'cam')}
  style={`left:calc(${cell.col} * var(--step-x));top:calc(${cell.row} * var(--step-y));width:calc(${cell.w} * var(--step-x) - 8px);height:calc(${cell.h} * var(--step-y) - 8px)`}
  tabindex="0" role="group" aria-roledescription="movable tile" aria-keyshortcuts="ArrowLeft ArrowRight ArrowUp ArrowDown Shift+ArrowLeft Shift+ArrowRight Shift+ArrowUp Shift+ArrowDown P O R Escape" aria-label={`${object?.name || participant?.name} ${share?.label || (object ? object.kind : 'camera')} tile`}
  onkeydown={key} onpointerdown={pointer}>
  <div class="content-home" bind:this={host}></div>
  {#if popped}<button class="placeholder" onclick={() => closePop?.()}>popped out · bring back</button>{/if}
  {#if object?.kind === 'terminal'}<span class="object-label">{object.name}</span>{/if}
  <!-- A phone keeps its tiles clear: the layout is chosen from the call's More. -->
  {#if !mobile}<div class="tile-actions">
    <button title="Pin" aria-label="Pin" aria-pressed={cell.pinned} onclick={onpin}><Icon name="pin" size={14} /></button>
    <button title="Pop out" aria-label="Pop out" onclick={pop}><Icon name="popout" size={14} /></button>
  </div>{/if}
  {#if !mobile && !popped}<button class="resize" title="Resize" aria-label="Resize" onpointerdown={(e) => { e.stopPropagation(); ondrag(e, true) }} onkeydown={(e) => { if (e.key.startsWith('Arrow')) { e.preventDefault(); e.stopPropagation(); onmove(e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0, e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0, true) } }}></button>{/if}
</div>

<style>
  .layout-tile { position: absolute; border-radius: var(--r-tile); background: var(--bg-3); min-width: 0; transition: left 140ms ease, top 140ms ease, width 140ms ease, height 140ms ease; }
  .layout-tile:not(.mobile) { touch-action: none; }
  .pinned { box-shadow: 0 0 0 2px var(--lamp-dim); }
  /* Plugin tiles (canvas, terminal) draw their own corners; match the call's. */
  .content-home :global(.canvas-tile), .content-home :global(.terminal-tile) { border-radius: inherit; }
  .layout-tile:focus-visible { outline: 2px solid var(--lamp); outline-offset: 3px; }
  .content-home { width: 100%; height: 100%; overflow: hidden; border-radius: inherit; display: grid; place-items: center; }
  .content-home :global(.call-content) { width: 100%; height: 100%; min-height: 0; position: relative; display: grid; place-items: center; }
  .content-home :global(.tile) { width: 100%; height: 100%; aspect-ratio: auto; }
  .content-home :global(.canvas-tile) { height: auto; aspect-ratio: 16/9; max-height: 100%; }
  .object-label { position: absolute; top: 8px; left: 76px; max-width: calc(100% - 88px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 4px 10px; background: var(--glass); backdrop-filter: blur(10px); border-radius: var(--r-pill); font-size: var(--fs-xs); }
  .tile-actions { position: absolute; z-index: 3; top: 8px; left: 8px; display: flex; gap: 2px; padding: 2px; opacity: 0; border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(10px); transition: opacity var(--t-fast); }
  .tile-actions button { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-pill); color: var(--ink-2); }
  @media (hover: hover) { .tile-actions button:hover { color: var(--ink); } }
  .tile-actions button[aria-pressed="true"] { color: var(--on-lamp); background: var(--lamp); }
  .layout-tile:focus-within .tile-actions { opacity: 1; }
  @media (hover: hover) { .layout-tile:hover .tile-actions { opacity: 1; } }
  .resize { position: absolute; z-index: 3; right: 3px; bottom: 3px; width: 16px; height: 16px; cursor: nwse-resize; opacity: 0; border-right: 3px solid var(--ink-2); border-bottom: 3px solid var(--ink-2); border-radius: 0 0 8px 0; }
  .layout-tile:focus-within .resize { opacity: 1; }
  @media (hover: hover) { .layout-tile:hover .resize { opacity: 1; } }
  .placeholder { position: absolute; inset: 0; width: 100%; font: var(--fs-xs) var(--body); color: var(--ink-3); border: 1px dashed var(--line); border-radius: inherit; }
  @media (prefers-reduced-motion: reduce) { .layout-tile { transition: none; } }
</style>
