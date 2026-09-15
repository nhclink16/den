<script lang="ts">
  import { store, type Store } from '../lib/store.svelte'
  import type { MusicQueue } from '../lib/types'
  let { roomId, owner = store }: { roomId: string; owner?: Store } = $props()
  let url = $state(''), busy = $state(false), error = $state(''), notice = $state('')
  const offered = $derived(/^https:\/\/(?:www\.|m\.|music\.)?(?:youtube\.com|youtu\.be)\//i.test(url.trim()))
  async function add(e: SubmitEvent) {
    e.preventDefault(); if (busy) return
    busy = true; error = ''; notice = ''
    try { owner.receiveMusic(await owner.api.post<MusicQueue>(`/rooms/${roomId}/music/queue`, { url: url.trim() })); url = ''; notice = 'Added to the queue.' }
    catch (e) { error = e instanceof Error ? e.message : 'Could not add this track.' }
    finally { busy = false }
  }
</script>
<form class="music-composer" onsubmit={add}>
  <label>YouTube URL<input type="url" bind:value={url} placeholder="Paste a YouTube link" required disabled={busy} aria-invalid={!!error} oninput={() => { error = ''; notice = '' }} /></label>
  {#if offered}<span class="offer">Queue this video for everyone in the call?</span>{/if}
  <button class="btn lit" disabled={busy}>{busy ? 'Adding…' : 'Queue track'}</button>
  <span class="status" role="status">{error || notice}</span>
</form>
<style>
  .music-composer { display: grid; gap: 8px; }
  label { display: grid; gap: 6px; color: var(--ink-2); font-size: 12px; }
  input { min-width: 0; width: 100%; min-height: 40px; padding: 8px 10px; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: var(--r); font: inherit; font-size: 16px; }
  .offer, .status { font-size: 12px; color: var(--ink-2); }
  .status:empty { display: none; }
  button { min-height: 40px; justify-content: center; }
</style>
