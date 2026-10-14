<script lang="ts">
  import { onDestroy } from 'svelte'
  import { store, instances } from '../lib/store.svelte'
  import { sounds, soundEvents } from '../lib/sounds'
  import { fetchBytes } from '../lib/api'
  import type { SoundEvent, SoundRef, SoundState, SoundPreferences, SoundPack } from '../lib/types'
  import SettingRow from './SettingRow.svelte'

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
  <h3 class="list-head">Your sounds</h3>
  <div class="slist">
    <SettingRow find="pack" label="Sound pack" hint="The server’s pack, Den’s own, or one you saved.">
      {#snippet control()}
        <select id="sound-pack" class="field" aria-label="Your sound pack" disabled={busy} value={prefs.pack_id || ''} onchange={e => save(p => { p.pack_id = e.currentTarget.value || null; p.overrides = {} })}>
          <option value="">Server default · {value.server_pack.name}</option>
          <option value="den">Den · built-in</option>
          {#each prefs.custom_packs as pack (pack.id)}<option value={pack.id}>{pack.name}</option>{/each}
        </select>
      {/snippet}
    </SettingRow>
    <SettingRow find="volume" label="Master volume">
      {#snippet control()}<input aria-label="Master volume" type="range" min="0" max="100" value={prefs.master_volume} disabled={busy} onchange={e => save(p => p.master_volume = +e.currentTarget.value)} /><output>{prefs.master_volume}%</output>{/snippet}
    </SettingRow>
    <SettingRow label="Test all" hint="Plays each sound in order, with a second between them.">
      {#snippet control()}<button class="btn" onclick={test}>{testing ? 'Stop test' : 'Test all'}</button>{/snippet}
    </SettingRow>
    <SettingRow label="Share or start over" hint="A pack file carries its sounds with it.">
      {#snippet control()}
        <label class="btn file-button">Import pack<input type="file" accept=".den-sounds.zip" aria-label="Import sound pack" disabled={busy} onchange={e => importPack(e.currentTarget)} /></label>
        <button class="btn" disabled={busy} onclick={exportPack}>Export pack</button>
        <button class="btn quiet" disabled={busy} onclick={() => save(p => { p.overrides = {}; p.pack_id = null })}>Use server defaults</button>
      {/snippet}
    </SettingRow>
  </div>
  <p class="list-foot"><b>Short sounds, by design.</b> WAV, MP3 or Ogg Vorbis. Up to 512 KiB and 5 seconds per file.</p>

  <h3 class="list-head">Each moment</h3>
  <div class="slist" data-find="events">
    {#each soundEvents as event (event.id)}
      {@const current = value.resolved[event.id]}
      {@const silent = current?.sound.type === 'silent'}
      <section class="event" class:playing={playing === event.id} aria-label={event.name}>
        <div class="event-heading"><h4>{event.name}</h4><span class="current">{label(current?.sound, event.id)}</span></div>
        <p class="desc">{event.description}</p>
        <div class="event-actions">
          <button class="btn" aria-label={`Play ${event.name}`} disabled={silent || busy || testing} onclick={() => sounds.play(event.id, owner)}>Play</button>
          <label class="btn file-button">Replace<input type="file" accept="audio/wav,audio/mpeg,audio/ogg,.wav,.mp3,.ogg" aria-label={`Replace ${event.name}`} disabled={busy} onchange={e => replace(event.id, e.currentTarget)} /></label>
          <label class="silence"><input class="toggle" type="checkbox" role="switch" checked={silent} disabled={busy} onchange={e => save(p => { if (e.currentTarget.checked) p.overrides[event.id] = { type: 'silent' }; else { delete p.overrides[event.id]; if (current?.sound.type === 'silent') p.overrides[event.id] = { type: 'builtin', name: event.id } } })} /> Silent</label>
          <label class="event-volume"><span class="sr-only">{event.name} volume</span><input aria-label={`${event.name} volume`} type="range" min="0" max="100" value={prefs.volumes[event.id] ?? 100} disabled={busy} onchange={e => save(p => p.volumes[event.id] = +e.currentTarget.value)} /><output>{prefs.volumes[event.id] ?? 100}%</output></label>
          {#if prefs.overrides[event.id]}<button class="reset" disabled={busy} onclick={() => save(p => { delete p.overrides[event.id] })} aria-label={`Reset ${event.name}`}>Reset</button>{/if}
        </div>
      </section>
    {/each}
  </div>

  <h3 class="list-head">Keep this set</h3>
  <div class="slist" data-find="keep">
    <SettingRow label="Save as a pack" hint="Save your choices as a named pack. Export it to share in any room." wide>
      {#snippet control()}
        <div class="tools"><input class="field" aria-label="Pack name" bind:value={name} maxlength="60" /><button class="btn" disabled={busy || !name.trim() || prefs.custom_packs.length >= 12} onclick={() => save(p => {
          const pack: SoundPack = { id: crypto.randomUUID(), name: name.trim(), sounds: { ...(p.pack_id === 'den' ? Object.fromEntries(soundEvents.map(e => [e.id, { type: 'builtin' as const, name: e.id }])) : p.custom_packs.find(v => v.id === p.pack_id)?.sounds), ...p.overrides } }
          p.custom_packs.push(pack); p.pack_id = pack.id; p.overrides = {}
        })}>Save as pack</button></div>
      {/snippet}
    </SettingRow>
    {#if prefs.pack_id && prefs.pack_id !== 'den'}<button class="action-row danger" disabled={busy} onclick={() => save(p => { p.custom_packs = p.custom_packs.filter(pack => pack.id !== p.pack_id); p.pack_id = null })}>Remove selected pack</button>{/if}
  </div>
  {#if owner.me?.role === 'admin'}
    <h3 class="list-head">Server default</h3>
    <div class="slist" data-find="server-pack">
      <SettingRow label={value.server_pack.name} hint="What new members hear. Personal packs and event choices always win.">
        {#snippet control()}
          <button class="btn" disabled={busy || !prefs.pack_id || prefs.pack_id === 'den'} onclick={serverPack}>Use selected pack</button>
          <button class="btn quiet" disabled={busy} onclick={() => action(async () => { owner.sounds = await owner.api.put('/settings/sounds', { id: 'den', name: 'Den', sounds: {} }); status = 'Server uses Den sounds.' })}>Reset to Den</button>
        {/snippet}
      </SettingRow>
    </div>
  {/if}
{:else}<p class="muted">Loading your sounds…</p>{/if}
<div class="feedback" aria-live="polite">{status}</div>
{#if error}<p role="alert" class="error">{error}</p>{/if}

<style>
  .intro { margin-bottom: 4px; max-width: 560px; }
  input[type=range] { accent-color: var(--accent); width: 150px; min-width: 65px; } output { width: 38px; font-size: var(--fs-xs); text-align: right; font-variant-numeric: tabular-nums; color: var(--ink-2); }
  select.field { width: auto; min-width: 0; max-width: 100%; }
  .tools { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; width: 100%; } .tools .field { flex: 1; min-width: 140px; }
  .file-button { position: relative; cursor: pointer; overflow: hidden; } .file-button input { position: absolute; inset: 0; opacity: 0; cursor: pointer; width: 100%; } .file-button:focus-within { outline: 2px solid var(--accent); outline-offset: 3px; }
  .event { padding: 12px 16px 14px; transition: background-color var(--t); } .event.playing { background-color: var(--sel); }
  .event-heading { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; } h4 { margin: 0; font-size: var(--fs-body); font-weight: 400; } .current { font-size: var(--fs-xs); color: var(--ink-3); text-align: end; }
  .desc { margin: 2px 0 10px; font-size: var(--fs-xs); color: var(--ink-2); }
  .event-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .silence { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); padding: 0 4px; cursor: pointer; }
  .event-volume { display: flex; flex: 1; align-items: center; gap: 8px; min-width: 140px; } .event-volume input { flex: 1; width: auto; } .reset { font-size: var(--fs-xs); color: var(--ink-2); padding: 7px; }
  .feedback { font-size: var(--fs-sm); color: var(--success); padding: 10px 4px 0; } .error { color: var(--danger); }
  @media (max-width: 550px) { .current { max-width: 48%; } .event-volume { flex-basis: 100%; } }
</style>
