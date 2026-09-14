<script lang="ts">
  import { onMount } from 'svelte'
  import type { ObjectSummary } from '../../lib/types'
  let { object, readonly = false }: { object: ObjectSummary; readonly?: boolean } = $props()
  let host: HTMLDivElement
  let error = $state('')
  let loading = $state(true)
  onMount(() => {
    let dead = false
    let destroy: (() => void) | undefined
    import('./editor').then(async ({ mountCanvas }) => {
      if (dead) return
      const cleanup = await mountCanvas(host, object, readonly, (message) => { error = message })
      if (dead) cleanup(); else { destroy = cleanup; loading = false }
    }).catch((err) => { error = err.message; loading = false })
    return () => { dead = true; destroy?.() }
  })
</script>
<div class="canvas-host" bind:this={host} data-testid={readonly ? 'canvas-tile-editor' : 'canvas-editor'}></div>
{#if loading}<span class="status">Opening canvas…</span>{/if}
{#if error}<span class="status error" role="alert">{error}</span>{/if}
<style>
  .canvas-host { position: absolute; inset: 0; }
  .status { position: absolute; left: 12px; top: 52px; max-width: calc(100% - 24px); padding: 6px 10px; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); z-index: 5; font-size: 13px; }
  .error { color: var(--ember); }
</style>
