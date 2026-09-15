<script lang="ts">
  import { fileUrl } from '../lib/upload'
  import { onMount, onDestroy } from 'svelte'
  import { instances } from '../lib/store.svelte'
  import { soundEvents, sounds } from '../lib/sounds'
  import type { Upload, SoundPack, SoundState, SoundEvent } from '../lib/types'
  let { upload, author }: { upload: Upload; author: string } = $props()
  const owner = instances.active
  const packFile = $derived(upload.filename.endsWith('.den-sounds.zip'))
  let pack = $state<SoundPack | null>(null), error = $state(''), status = $state(''), busy = $state(false), playing = $state(false)
  let event = $state<SoundEvent>('message')
  onMount(async () => { try { pack = await owner.api.get(`/uploads/${upload.id}/sounds`) } catch (e) { error = (e as Error).message } })
  onDestroy(() => { if (playing) sounds.stop() })
  async function play() {
    if (playing) { sounds.stop(); playing = false; return }
    if (!pack) return
    playing = true; error = ''
    try { await sounds.test(owner, () => {}, pack, upload.id) } catch (e) { error = (e as Error).message } finally { playing = false }
  }
  async function install() {
    busy = true; error = ''
    try { owner.sounds = await owner.api.post<SoundState>(`/uploads/${upload.id}/sounds`, { event: packFile ? null : event }); status = packFile ? 'Added to your sounds. Other events kept their choices.' : `Saved for ${soundEvents.find(e => e.id === event)!.name}.` } catch (e) { error = (e as Error).message } finally { busy = false }
  }
</script>
<article class="sound-card" aria-label={packFile ? 'Sound pack' : 'Sound'}>
  <span class="eyebrow">{packFile ? 'Sound pack' : 'Sound'}</span>
  <h3>{pack?.name || upload.filename}</h3>
  <p class="muted small">Posted by {author}{#if packFile && pack} · {Object.keys(pack.sounds).length} events{/if}</p>
  <div class="actions"><button class="btn" onclick={play} disabled={!pack}>{playing ? 'Stop' : 'Play'}</button>
    {#if packFile}<button class="btn lit" onclick={install} disabled={!pack || busy}>Add to my sounds</button>
    {:else}<label><span class="sr-only">Use sound for</span><select class="field" aria-label="Use sound for" bind:value={event}>{#each soundEvents as e}<option value={e.id}>{e.name}</option>{/each}</select></label><button class="btn lit" onclick={install} disabled={!pack || busy}>Use for…</button>{/if}
  </div>
  <p class="faint small">Playing is a preview. Your choices change only when you add it.</p>
  {#if error && !pack && !packFile}<audio controls preload="none" src={fileUrl(upload.id)}></audio>{/if}
  {#if status}<p class="small" role="status">{status}</p>{/if}{#if error}<p class="error small" role="alert">{error}</p>{/if}
</article>
<style>
  .sound-card { border: 1px solid var(--line); border-radius: var(--r-lg); padding: 16px 18px; background: var(--bg-2); max-width: 480px; }
  h3 { margin: 6px 0; font-size: 17px; overflow-wrap: anywhere; } .small { font-size: 12px; margin: 8px 0; } .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 14px 0 10px; } .actions .field { max-width: 170px; } .error { color: var(--danger); } audio { max-width: 100%; }
</style>
