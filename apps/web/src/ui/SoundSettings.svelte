<script lang="ts">
  import { onDestroy } from 'svelte'
  import { store, instances } from '../lib/store.svelte'
  import { sounds, soundEvents } from '../lib/sounds'
  import { fetchBytes } from '../lib/api'
  import type { SoundEvent, SoundRef, SoundState, SoundPreferences, SoundPack } from '../lib/types'

  const owner = instances.active
  let busy = $state(false), error = $state(''), status = $state(''), name = $state('My sound pack')
  let playing = $state<SoundEvent | null>(null), testing = $state(false)
  const value = $derived(store.sounds)
  const prefs = $derived(value?.preferences)
  async function action(fn: () => Promise<void>) {
    busy = true; error = ''; status = ''
    try { await fn() } catch (e) { error = (e as Error).message } finally { busy = false }
  }
  async function save(change: (p: SoundPreferences) => void) {
    await action(async () => {
      const p: SoundPreferences = JSON.parse(JSON.stringify(owner.sounds!.preferences))
      change(p); await owner.saveSounds(p); status = 'Saved to your account.'
    })
  }
  function label(ref: SoundRef | undefined, event: SoundEvent) {
    if (!ref) return 'Den'
    if (ref.type === 'silent') return 'Silent'
    if (ref.type === 'builtin') return `Den · ${soundEvents.find(e => e.id === ref.name)?.name || ref.name}`
    const pack = [...(prefs?.custom_packs || []), ...(value ? [value.server_pack] : [])].find(p => { const sound = p.sounds[event]; return sound?.type === 'upload' && sound.id === ref.id })
    return pack?.name || ref.id.replace(/-[0-9a-f]{8}$/, '').slice(0, 36)
  }
  async function replace(event: SoundEvent, input: HTMLInputElement) {
    const file = input.files?.[0]; if (!file) return
    await action(async () => {
      if (file.size > 512 * 1024) throw Error('Choose a file of 512 KiB or less.')
      const type = file.name.toLowerCase().endsWith('.wav') ? 'audio/wav' : file.name.toLowerCase().endsWith('.mp3') ? 'audio/mpeg' : file.type
      const id = `${file.name.replace(/[^a-zA-Z0-9._-]/g, '-').replace(/^[^a-zA-Z0-9]+/, '').slice(0, 48) || 'sound'}-${crypto.randomUUID().slice(0, 8)}`
      const ref = await owner.api.putRaw<SoundRef>(`/users/me/sounds/${id}`, file, { 'content-type': type })
      await owner.saveSounds({ ...owner.sounds!.preferences, overrides: { ...owner.sounds!.preferences.overrides, [event]: ref } })
      status = `${soundEvents.find(e => e.id === event)!.name} replaced.`
    }); input.value = ''
  }
  async function importPack(input: HTMLInputElement) {
    const file = input.files?.[0]; if (!file) return
    await action(async () => { owner.sounds = await owner.api.putRaw<SoundState>('/users/me/sounds/import', file, { 'content-type': 'application/zip' }); status = 'Pack added. Events it leaves out keep their current sounds.' }); input.value = ''
  }
  async function exportPack() {
    await action(async () => {
      const bytes = await fetchBytes(owner.origin, '/users/me/sounds/export')
      const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }))
      const a = document.createElement('a'); a.href = url; a.download = 'My-sounds.den-sounds.zip'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000)
      status = 'Pack exported with its audio files.'
    })
  }
  async function test() {
    if (testing) { sounds.stop(); testing = false; playing = null; return }
    testing = true; error = ''
    try { await sounds.test(owner, event => playing = event) } catch (e) { error = (e as Error).message } finally { testing = false }
  }
  async function serverPack() {
    await action(async () => {
      const p = owner.sounds!.preferences
      const pack = p.custom_packs.find(pack => pack.id === p.pack_id)
      if (!pack) throw Error('Save and select a pack first.')
      owner.sounds = await owner.api.put<SoundState>('/settings/sounds', pack)
      status = 'Server pack saved. Each member keeps their own overrides.'
    })
  }
  onDestroy(() => sounds.stop())
</script>

<h2 class="display">Sounds</h2>
<p class="muted intro">A small voice for each moment. Start with the server’s pack, or make the set your own.</p>
{#if value && prefs}
  <div class="pack-panel">
    <div class="pack-top"><div><span class="eyebrow">Your sound pack</span><label class="sr-only" for="sound-pack">Your sound pack</label>
      <select id="sound-pack" class="field" disabled={busy} value={prefs.pack_id || ''} onchange={e => save(p => { p.pack_id = e.currentTarget.value || null; p.overrides = {} })}>
        <option value="">Server default · {value.server_pack.name}</option>
        <option value="den">Den · built-in</option>
        {#each prefs.custom_packs as pack (pack.id)}<option value={pack.id}>{pack.name}</option>{/each}
      </select>
    </div><button class="btn lit" onclick={test}>{testing ? 'Stop test' : 'Test all'}</button></div>
    <label class="volume">Master volume <input aria-label="Master volume" type="range" min="0" max="100" value={prefs.master_volume} disabled={busy} onchange={e => save(p => p.master_volume = +e.currentTarget.value)} /><output>{prefs.master_volume}%</output></label>
    <p class="faint small">Test all plays each event in order, with a second between sounds.</p>
    <div class="tools">
      <label class="btn file-button">Import pack<input type="file" accept=".den-sounds.zip" aria-label="Import sound pack" disabled={busy} onchange={e => importPack(e.currentTarget)} /></label>
      <button class="btn" disabled={busy} onclick={exportPack}>Export pack</button>
      <button class="btn quiet" disabled={busy} onclick={() => save(p => { p.overrides = {}; p.pack_id = null })}>Use server defaults</button>
    </div>
  </div>
  <div class="limits"><b>Short sounds, by design.</b> WAV, MP3 or Ogg Vorbis. Up to 512 KiB and 5 seconds per file.</div>
  <div class="events">
    {#each soundEvents as event (event.id)}
      {@const current = value.resolved[event.id]}
      {@const silent = current?.sound.type === 'silent'}
      <section class="event" class:playing={playing === event.id} aria-label={event.name}>
        <div class="event-heading"><h3>{event.name}</h3><span class="current">{label(current?.sound, event.id)}</span></div>
        <p class="muted small">{event.description}</p>
        <div class="event-actions">
          <button class="btn" aria-label={`Play ${event.name}`} disabled={silent || busy || testing} onclick={() => sounds.play(event.id, owner)}>Play</button>
          <label class="btn file-button">Replace<input type="file" accept="audio/wav,audio/mpeg,audio/ogg,.wav,.mp3,.ogg" aria-label={`Replace ${event.name}`} disabled={busy} onchange={e => replace(event.id, e.currentTarget)} /></label>
          <label class="silence"><input type="checkbox" checked={silent} disabled={busy} onchange={e => save(p => { if (e.currentTarget.checked) p.overrides[event.id] = { type: 'silent' }; else { delete p.overrides[event.id]; if (current?.sound.type === 'silent') p.overrides[event.id] = { type: 'builtin', name: event.id } } })} /> Silent</label>
          <label class="event-volume"><span class="sr-only">{event.name} volume</span><input aria-label={`${event.name} volume`} type="range" min="0" max="100" value={prefs.volumes[event.id] ?? 100} disabled={busy} onchange={e => save(p => p.volumes[event.id] = +e.currentTarget.value)} /><output>{prefs.volumes[event.id] ?? 100}%</output></label>
          {#if prefs.overrides[event.id]}<button class="reset" disabled={busy} onclick={() => save(p => { delete p.overrides[event.id] })} aria-label={`Reset ${event.name}`}>Reset</button>{/if}
        </div>
      </section>
    {/each}
  </div>
  <section class="save-pack">
    <h3>Keep this set</h3><p class="muted small">Save your choices as a named pack. Export it to share in any room.</p>
    <div class="tools"><input class="field" aria-label="Pack name" bind:value={name} maxlength="60" /><button class="btn" disabled={busy || !name.trim() || prefs.custom_packs.length >= 12} onclick={() => save(p => {
      const pack: SoundPack = { id: crypto.randomUUID(), name: name.trim(), sounds: { ...(p.pack_id === 'den' ? Object.fromEntries(soundEvents.map(e => [e.id, { type: 'builtin' as const, name: e.id }])) : p.custom_packs.find(v => v.id === p.pack_id)?.sounds), ...p.overrides } }
      p.custom_packs.push(pack); p.pack_id = pack.id; p.overrides = {}
    })}>Save as pack</button></div>
    {#if prefs.pack_id && prefs.pack_id !== 'den'}<button class="btn quiet" disabled={busy} onclick={() => save(p => { p.custom_packs = p.custom_packs.filter(pack => pack.id !== p.pack_id); p.pack_id = null })}>Remove selected pack</button>{/if}
  </section>
  {#if owner.me?.role === 'admin'}
    <section class="server"><h3>Server default</h3><p class="muted small"><b>{value.server_pack.name}</b> is what new members hear. Personal packs and event choices always win.</p>
    <button class="btn" disabled={busy || !prefs.pack_id || prefs.pack_id === 'den'} onclick={serverPack}>Use selected pack for this server</button>
    <button class="btn quiet" disabled={busy} onclick={() => action(async () => { owner.sounds = await owner.api.put('/settings/sounds', { id: 'den', name: 'Den', sounds: {} }); status = 'Server uses Den sounds.' })}>Reset server to Den</button></section>
  {/if}
{:else}<p class="muted">Loading your sounds…</p>{/if}
<div class="feedback" aria-live="polite">{status}</div>
{#if error}<p role="alert" class="error">{error}</p>{/if}

<style>
  h2 { font-size: 26px; margin: 0 0 6px; } h3 { font-size: 15px; margin: 0; } .intro { margin-bottom: 22px; max-width: 560px; }
  .pack-panel { padding: 18px; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r-lg); }
  .pack-top { display: flex; align-items: flex-end; gap: 14px; } .pack-top > div { flex: 1; } .eyebrow { display: block; margin-bottom: 8px; }
  .volume { display: flex; align-items: center; gap: 14px; margin-top: 18px; font-size: 13px; } input[type=range] { accent-color: var(--accent); min-width: 65px; flex: 1; } output { width: 38px; font-size: 12px; text-align: right; font-variant-numeric: tabular-nums; color: var(--ink-2); }
  .small { font-size: 13px; } .tools { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; } .tools .field { flex: 1; min-width: 140px; }
  .file-button { position: relative; cursor: pointer; overflow: hidden; } .file-button input { position: absolute; inset: 0; opacity: 0; cursor: pointer; width: 100%; } .file-button:focus-within { outline: 2px solid var(--accent); outline-offset: 3px; }
  .limits { font-size: 12px; color: var(--ink-2); line-height: 1.6; padding: 18px 0 8px; } .limits b { color: var(--ink); }
  .event { padding: 18px 0; border-bottom: 1px solid var(--line); } .event.playing { background: var(--accent-glow); outline: 6px solid var(--accent-glow); }
  .event-heading { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; } .current { font-size: 12px; color: var(--ink-3); } .event p { margin: 5px 0 12px; }
  .event-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; } .event-actions .btn { font-size: 12px; min-height: 32px; }
  .silence { display: flex; align-items: center; gap: 5px; font-size: 12px; padding: 6px; } .silence input { accent-color: var(--accent); }
  .event-volume { display: flex; flex: 1; align-items: center; gap: 8px; min-width: 120px; margin-left: 8px; } .reset { font-size: 12px; color: var(--ink-2); padding: 7px; }
  .save-pack, .server { padding-top: 24px; } .feedback { font-size: 13px; color: var(--success); padding-top: 14px; } .error { color: var(--danger); }
  @media (max-width: 550px) { .pack-top { align-items: stretch; flex-direction: column; } .current { max-width: 48%; text-align: right; } .event-volume { flex-basis: 100%; margin-left: 0; } }
</style>
