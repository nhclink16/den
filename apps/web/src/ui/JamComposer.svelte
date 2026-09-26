<script lang="ts">
  // The paste box in the call's Music menu. The server only accepts it from
  // someone in the call, and ends any Jam already live there.
  import { store, type Store } from '../lib/store.svelte'
  import { isJamLink, startJam } from '../lib/jam'
  let { roomId, owner = store, label = 'Start the Jam' }: { roomId: string; owner?: Store; label?: string } = $props()
  let url = $state(''), busy = $state(false), error = $state('')
  let input: HTMLInputElement
  const ready = $derived(isJamLink(url))
  async function start(e: SubmitEvent) {
    e.preventDefault(); if (busy) return
    if (!ready) {
      error = 'Paste a Spotify Jam link, such as open.spotify.com/jam/…'
      input.focus()
      return
    }
    busy = true; error = ''
    try { await startJam(owner, roomId, url); url = '' }
    catch (e) { error = e instanceof Error ? e.message : 'Could not start that Jam.' }
    finally { busy = false }
  }
</script>
<form class="jam-composer" onsubmit={start}>
  <label>Spotify Jam link<input bind:this={input} type="url" bind:value={url} placeholder="https://open.spotify.com/jam/…" required disabled={busy} aria-invalid={error ? 'true' : undefined} aria-describedby="jam-link-status" oninput={() => (error = '')} /></label>
  <button class="btn" disabled={busy}>{busy ? 'Starting…' : label}</button>
  <span id="jam-link-status" class="status" role="status">{error || (url && !ready ? 'Use a Spotify Jam share link.' : '')}</span>
</form>
<style>
  .jam-composer { display: grid; gap: 8px; }
  label { display: grid; gap: 6px; color: var(--ink-2); font-size: 12px; }
  input { min-width: 0; width: 100%; min-height: 40px; padding: 8px 10px; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: var(--r); font: inherit; font-size: 16px; }
  .status { font-size: 12px; color: var(--ink-2); }
  .status:empty { display: none; }
  button { min-height: 40px; justify-content: center; }
</style>
