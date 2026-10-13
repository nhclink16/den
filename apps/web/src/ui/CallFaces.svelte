<script lang="ts">
  // Who's in a call, as overlapping pictures. The hangout card and the call pill
  // both draw it, so they read as one thing. Someone talking gets the accent ring.
  import type { Store } from '../lib/store.svelte'
  import Avatar from './Avatar.svelte'
  let { faces, size = 28, max = 3, instance }: { faces: { userId: string; speaking?: boolean }[]; size?: number; max?: number; instance?: Store } = $props()
  const more = $derived(faces.length > max + 1 ? faces.length - max : 0)
</script>

<span class="faces" aria-hidden="true">
  {#each more ? faces.slice(0, max) : faces as f (f.userId)}<span class:speaking={f.speaking}><Avatar userId={f.userId} {size} {instance} presence={false} /></span>{/each}
  {#if more}<small>+{more}</small>{/if}
</span>

<style>
  /* --ring is whatever the faces sit on, so the gap between pictures reads as a cut. */
  .faces { display: flex; align-items: center; padding-left: 7px; flex: none; }
  .faces > span { margin-left: -7px; display: flex; border-radius: var(--avatar-r, 50%); box-shadow: 0 0 0 2px var(--ring, var(--bg-3)); }
  .faces > span.speaking { position: relative; z-index: 1; box-shadow: 0 0 0 2px var(--ring, var(--bg-3)), 0 0 0 4px var(--lamp); }
  small { margin-left: 4px; font-size: var(--fs-xs); color: var(--ink-2); }
</style>
