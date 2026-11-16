<script lang="ts">
  import { mediaUrl } from '../../lib/native'
  import type { ObjectProps } from '..'
  import { objects, openObject } from '../../lib/objects.svelte'
  import Avatar from '../../ui/Avatar.svelte'
  import Icon from '../../ui/Icon.svelte'
  import { edited } from './edited'
  let { object }: ObjectProps = $props()
  const people = $derived(objects.presence[object.id] || [])
  // "edited 5 min ago" moves on by itself.
  let now = $state(new Date())
  $effect(() => { const timer = setInterval(() => (now = new Date()), 60_000); return () => clearInterval(timer) })
</script>
<button class="canvas-card" title="Open canvas" onclick={() => openObject(object)} data-object-id={object.id}>
  <!-- An editor that has it open draws the preview, so a canvas an agent made has none yet. Say so, or it reads as blank. -->
  <span class="preview">{#if object.thumbnail_url}<img src={mediaUrl(object.thumbnail_url)} alt="Canvas preview" />{:else}<span class="empty"><Icon name="canvas" size={24} />No preview yet</span>{/if}</span>
  <span class="caption"><span class="title"><span class="eyebrow">Canvas <span class="when">· edited {edited(object.updated_at, now)}</span></span><span class="name">{object.name || 'Untitled canvas'}</span></span>
    {#if people.length}<span class="people" aria-label={`${people.length} on this canvas`}><span class="live"></span>{#each people as userId}<Avatar {userId} size={20} />{/each}</span>{/if}
  </span>
</button>
<style>
  .canvas-card { display: block; width: 320px; max-width: 100%; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r-lg); overflow: hidden; text-align: left; margin: 6px 0; }
  @media (hover: hover) { .canvas-card:hover { border-color: var(--ink-3); } }
  .preview { display: block; aspect-ratio: 16/9; background: radial-gradient(var(--line) 1px, transparent 1px) 0 0 / 16px 16px var(--bg-3); }
  img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .empty { display: grid; place-content: center; justify-items: center; gap: var(--s2); height: 100%; color: var(--ink-3); font-size: var(--fs-sm); }
  .when { font-weight: 400; }
  .caption { display: flex; align-items: center; padding: 10px 12px; gap: 10px; }
  .title { flex: 1; min-width: 0; display: grid; }
  .name { font: var(--fs-body) var(--display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* The avatar stack, drawn the same way on the terminal card. */
  .people { display: flex; align-items: center; padding-left: 6px; }
  .people :global(.avatar) { margin-left: -6px; box-shadow: 0 0 0 2px var(--bg-2); }
  .people :global(.dot) { display: none; }
  .live { width: 7px; height: 7px; border-radius: 50%; background: var(--lamp); box-shadow: 0 0 8px var(--lamp); margin-right: 12px; }
</style>
