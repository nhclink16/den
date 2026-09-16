<script lang="ts">
  // Voice rooms have no message composer, so a Jam link needs its own way in.
  // Text rooms get the offer inside Composer instead.
  import { store, type Store } from '../lib/store.svelte'
  import { isJamLink, startJam } from '../lib/jam'
  let { roomId, owner = store }: { roomId: string; owner?: Store } = $props()
  let url = $state(''), busy = $state(false), error = $state('')
  const ready = $derived(isJamLink(url))
  async function pin(e: SubmitEvent) {
    e.preventDefault(); if (busy) return
    busy = true; error = ''
    try { await startJam(owner, roomId, url); url = '' }
    catch (e) { error = e instanceof Error ? e.message : 'Could not pin that Jam.' }
    finally { busy = false }
  }
</script>
<form class="jam-composer" onsubmit={pin}>
  <label>Spotify Jam link<input type="url" bind:value={url} placeholder="Paste a Jam link from Spotify" required disabled={busy} aria-invalid={!!error} oninput={() => (error = '')} /></label>
  <button class="btn" disabled={busy || !ready}>{busy ? 'Pinning…' : 'Pin a Jam'}</button>
  <span class="status" role="status">{error || (url && !ready ? 'That is not a Spotify Jam link.' : '')}</span>
</form>
<style>
  .jam-composer { display: grid; gap: 8px; }
  label { display: grid; gap: 6px; color: var(--ink-2); font-size: 12px; }
  input { min-width: 0; width: 100%; min-height: 40px; padding: 8px 10px; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: var(--r); font: inherit; font-size: 16px; }
  .status { font-size: 12px; color: var(--ink-2); }
  .status:empty { display: none; }
  button { min-height: 40px; justify-content: center; }
</style>
