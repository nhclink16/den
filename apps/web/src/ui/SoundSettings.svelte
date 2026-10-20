<script module lang="ts">
  // Every write copies the preferences it changes, so writes go one at a time, across visits to the page too:
  // one still in flight can't be undone by the next (a volume, a choice, a new pack).
  let writes: Promise<unknown> = Promise.resolve()
  // Read anything from the event before calling in: a queued write runs after the event is over.
  function queued<T>(fn: () => Promise<T>): Promise<T> { const next = writes.then(fn); writes = next.catch(() => {}); return next }
</script>

<script lang="ts">
  import { onDestroy } from 'svelte'
  import { store, instances } from '../lib/store.svelte'
  import { sounds, soundEvents } from '../lib/sounds'
  import { fetchBytes } from '../lib/api'
  import { router } from '../lib/router.svelte'
  import { shell } from '../lib/shell-call'
  import type { SoundEvent, SoundRef, SoundState, SoundPreferences, SoundPack } from '../lib/types'
  import SettingRow from './SettingRow.svelte'
  import FormDialog from './FormDialog.svelte'
  import Icon from './Icon.svelte'

  /**
   * The list: the pack, the volume, then one row per moment, like iOS's Sounds. `moment` is one moment's own
   * page: a checkmark list of sounds that plays each one you pick, its volume, and a file of your own.
   */
  let { moment }: { moment?: SoundEvent } = $props()
  const owner = instances.active
  let busy = $state(false), error = $state(''), status = $state(''), name = $state('My sound pack'), naming = $state(false)
  let testing = $state(false)
  let fileInput = $state<HTMLInputElement>()
  const value = $derived(store.sounds)
  const prefs = $derived(value?.preferences)
  // The iPhone app has no terminals to ring.
  const events = soundEvents.filter(e => !(shell && e.id === 'terminal_bell'))
  const event = $derived(soundEvents.find(e => e.id === moment))
  // A write queued by one account never runs for the next one signed in after it.
  const mine = (fn: () => Promise<void>) => { const who = owner.me?.id; return () => (owner.me?.id === who ? fn() : Promise.resolve()) }
  const action = (fn: () => Promise<void>) => queued(mine(() => run(fn)))
  async function run(fn: () => Promise<void>) {
    busy = true; error = ''; status = ''
    try { await fn() } catch (e) { error = (e as Error).message } finally { busy = false }
  }
  async function save(change: (p: SoundPreferences) => void) {
    await action(async () => {
      const p: SoundPreferences = JSON.parse(JSON.stringify(owner.sounds!.preferences))
      change(p); await owner.saveSounds(p)
    })
  }
  const builtinName = (id: string) => soundEvents.find(e => e.id === id)?.name || id
  function label(ref: SoundRef | undefined, event: SoundEvent) {
    if (!ref) return 'Den'
    if (ref.type === 'silent') return 'None'
    if (ref.type === 'builtin') return builtinName(ref.name)
    const pack = [...(prefs?.custom_packs || []), ...(value ? [value.server_pack] : [])].find(p => { const sound = p.sounds[event]; return sound?.type === 'upload' && sound.id === ref.id })
    return pack?.name || ref.id.replace(/-[0-9a-f]{8}$/, '').slice(0, 36)
  }
  /** What the pack you're on plays for a moment, before any choice of your own. */
  function packSound(event: SoundEvent): SoundRef | undefined {
    if (!value || !prefs) return undefined
    const pack = prefs.pack_id === 'den' ? null : prefs.custom_packs.find(p => p.id === prefs.pack_id) ?? (prefs.pack_id ? null : value.server_pack)
    return pack ? pack.sounds[event] : { type: 'builtin', name: event }
  }
  const packName = $derived(!value || !prefs ? '' : prefs.pack_id === 'den' ? 'Den' : prefs.custom_packs.find(p => p.id === prefs.pack_id)?.name ?? value.server_pack.name)
  /**
   * Choosing a sound plays it, as iOS does. The radios stay enabled while it saves (disabling them would drop
   * keyboard focus mid-arrow); the write queue keeps the choices in order.
   */
  async function choose(event: SoundEvent, ref: SoundRef | null) {
    sounds.stop()
    await save(p => { if (ref) p.overrides[event] = ref; else delete p.overrides[event] })
    if (!error && ref?.type !== 'silent') void sounds.play(event, owner)
  }
  const picked = (event: SoundEvent, ref: SoundRef | null) => {
    const o = prefs?.overrides[event]
    if (!ref) return !o
    return !!o && o.type === ref.type && (o.type !== 'builtin' || (ref.type === 'builtin' && o.name === ref.name))
  }
  function open(event: SoundEvent) { router.go(`/settings/sounds/${event}`, false, '/settings/sounds') }

  async function replace(event: SoundEvent, input: HTMLInputElement) {
    const file = input.files?.[0]; if (!file) return
    await action(async () => {
      if (file.size > 512 * 1024) throw Error('Choose a file of 512 KiB or less.')
      const type = file.name.toLowerCase().endsWith('.wav') ? 'audio/wav' : file.name.toLowerCase().endsWith('.mp3') ? 'audio/mpeg' : file.type
      const id = `${file.name.replace(/[^a-zA-Z0-9._-]/g, '-').replace(/^[^a-zA-Z0-9]+/, '').slice(0, 48) || 'sound'}-${crypto.randomUUID().slice(0, 8)}`
      const ref = await owner.api.putRaw<SoundRef>(`/users/me/sounds/${id}`, file, { 'content-type': type })
      await owner.saveSounds({ ...owner.sounds!.preferences, overrides: { ...owner.sounds!.preferences.overrides, [event]: ref } })
      status = `${soundEvents.find(e => e.id === event)!.name} uses your file.`
    }); input.value = ''
    if (!error) void sounds.play(event, owner)
  }
  async function importPack(input: HTMLInputElement) {
    const file = input.files?.[0]; if (!file) return
    await action(async () => { owner.sounds = await owner.api.putRaw<SoundState>('/users/me/sounds/import', file, { 'content-type': 'application/zip' }); status = 'Pack added. Moments it leaves out keep their sounds.' }); input.value = ''
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
    if (testing) { sounds.stop(); testing = false; return }
    testing = true; error = ''
    try { await sounds.test(owner, () => {}) } catch (e) { error = (e as Error).message } finally { testing = false }
  }
  async function savePack() {
    // Everything else waits, even if the dialog is cancelled mid-save: another save would drop the new pack.
    busy = true
    const named = name.trim()
    try { await queued(mine(() => writePack(named))) } finally { busy = false }
  }
  async function writePack(named: string) {
    const p: SoundPreferences = JSON.parse(JSON.stringify(owner.sounds!.preferences))
    if (p.custom_packs.length >= 12) throw Error('You have 12 packs. Remove one first.')
    const pack: SoundPack = { id: crypto.randomUUID(), name: named, sounds: { ...(p.pack_id === 'den' ? Object.fromEntries(soundEvents.map(e => [e.id, { type: 'builtin' as const, name: e.id }])) : p.custom_packs.find(v => v.id === p.pack_id)?.sounds), ...p.overrides } }
    p.custom_packs.push(pack); p.pack_id = pack.id; p.overrides = {}
    await owner.saveSounds(p)
    status = `Saved as ${pack.name}.`
  }
  /** `id`: the pack showing when the button was tapped. */
  async function serverPack(id: string | null) {
    await action(async () => {
      const p = owner.sounds!.preferences
      const pack = p.custom_packs.find(pack => pack.id === id)
      if (!pack) throw Error('Save and select a pack first.')
      owner.sounds = await owner.api.put<SoundState>('/settings/sounds', pack)
      status = 'Server pack saved. Each member keeps their own choices.'
    })
  }
  onDestroy(() => sounds.stop())
</script>

{#if !value || !prefs}
  <h2 class="display">Sounds</h2>
  <p class="muted">Loading your sounds…</p>
{:else if event}
  {@const current = value.resolved[event.id]}
  <h2 class="display">{event.name}</h2>
  <div class="slist gap-top" role="radiogroup" aria-label={`Sound for ${event.name}`}>
    {@render choice(event.id, { type: 'silent' }, 'None')}
    {@render choice(event.id, null, 'Default', `${packName} · ${label(packSound(event.id), event.id)}`)}
    {#if prefs.overrides[event.id]?.type === 'upload'}{@render choice(event.id, prefs.overrides[event.id]!, label(prefs.overrides[event.id], event.id))}{/if}
  </div>
  <p class="list-foot">{event.description}</p>
  <h3 class="list-head">Den's sounds</h3>
  <div class="slist" role="radiogroup" aria-label="Den's sounds">
    {#each events as e (e.id)}{@render choice(event.id, { type: 'builtin', name: e.id }, e.name)}{/each}
  </div>
  <div class="slist gap-top">
    <label class="action-row file">Choose a File…<input type="file" accept="audio/wav,audio/mpeg,audio/ogg,.wav,.mp3,.ogg" aria-label={`Choose a file for ${event.name}`} disabled={busy} onchange={e => replace(event.id, e.currentTarget)} /></label>
  </div>
  <p class="list-foot">WAV, MP3 or Ogg Vorbis, up to 512 KiB and 5 seconds.</p>
  <h3 class="list-head">Volume</h3>
  <div class="slist">
    <SettingRow label="Volume">
      {#snippet control()}<input aria-label={`${event.name} volume`} type="range" min="0" max="100" value={prefs.volumes[event.id] ?? 100} disabled={busy || current?.sound.type === 'silent'} onchange={e => { const v = +e.currentTarget.value, id = event.id; void save(p => p.volumes[id] = v).then(() => sounds.play(id, owner)) }} /><output>{prefs.volumes[event.id] ?? 100}%</output>{/snippet}
    </SettingRow>
  </div>
{:else}
  <h2 class="display">Sounds</h2>
  <div class="slist gap-top">
    <SettingRow find="pack" label="Sound pack">
      {#snippet control()}
        <select id="sound-pack" class="field" aria-label="Your sound pack" disabled={busy} value={prefs.pack_id || ''} onchange={e => { const id = e.currentTarget.value || null; save(p => { p.pack_id = id; p.overrides = {} }) }}>
          <option value="">Server default · {value.server_pack.name}</option>
          <option value="den">Den · built-in</option>
          {#each prefs.custom_packs as pack (pack.id)}<option value={pack.id}>{pack.name}</option>{/each}
        </select>
      {/snippet}
    </SettingRow>
    <SettingRow find="volume" label="Volume">
      {#snippet control()}<input aria-label="Master volume" type="range" min="0" max="100" value={prefs.master_volume} disabled={busy} onchange={e => { const v = +e.currentTarget.value; save(p => p.master_volume = v) }} /><output>{prefs.master_volume}%</output>{/snippet}
    </SettingRow>
    <button class="action-row" onclick={test}>{testing ? 'Stop' : 'Play All'}</button>
  </div>

  <h3 class="list-head">Each moment</h3>
  <nav class="slist" data-find="events" aria-label="Each moment">
    {#each events as e (e.id)}
      {@const chosen = prefs.overrides[e.id] ? label(prefs.overrides[e.id], e.id) : 'Default'}
      <a class="nav-row" href="/settings/sounds/{e.id}" aria-label={`${e.name}, ${chosen}`} onclick={ev => { ev.preventDefault(); open(e.id) }}>
        <span class="grow">{e.name}</span><span class="val">{chosen}</span><span class="chev"><Icon name="chevron" size={14} /></span>
      </a>
    {/each}
  </nav>

  <h3 class="list-head">Packs</h3>
  <div class="slist" data-find="keep">
    <button class="action-row" disabled={busy || prefs.custom_packs.length >= 12} onclick={() => { name = 'My sound pack'; naming = true }}>Save as a Pack…</button>
    <label class="action-row file">Import a Pack…<input type="file" accept=".den-sounds.zip" aria-label="Import sound pack" disabled={busy} onchange={e => importPack(e.currentTarget)} /></label>
    <button class="action-row" disabled={busy} onclick={exportPack}>Export My Sounds</button>
    <button class="action-row" disabled={busy} onclick={() => save(p => { p.overrides = {}; p.pack_id = null })}>Use Server Defaults</button>
    {#if prefs.pack_id && prefs.pack_id !== 'den'}<button class="action-row danger" disabled={busy} onclick={() => { const id = prefs.pack_id; void save(p => { p.custom_packs = p.custom_packs.filter(pack => pack.id !== id); if (p.pack_id === id) p.pack_id = null }) }}>Remove {packName}</button>{/if}
  </div>
  <p class="list-foot">A pack file carries its sounds with it: share it in any room.</p>
  {#if owner.me?.role === 'admin'}
    <h3 class="list-head">Server default</h3>
    <div class="slist" data-find="server-pack">
      <SettingRow label={value.server_pack.name} hint="What new members hear. Their own choices always win.">
        {#snippet control()}
          <button class="btn" disabled={busy || !prefs.pack_id || prefs.pack_id === 'den'} onclick={() => serverPack(prefs.pack_id)}>Use selected pack</button>
          <button class="btn quiet" disabled={busy} onclick={() => action(async () => { owner.sounds = await owner.api.put('/settings/sounds', { id: 'den', name: 'Den', sounds: {} }); status = 'Server uses Den sounds.' })}>Reset to Den</button>
        {/snippet}
      </SettingRow>
    </div>
  {/if}
  <FormDialog bind:open={naming} title="Save as a Pack" action="Save" disabled={!name.trim()} onsubmit={savePack}>
    <input class="field" aria-label="Pack name" bind:value={name} maxlength="60" />
  </FormDialog>
{/if}
<div class="feedback" aria-live="polite">{status}</div>
{#if error}<p role="alert" class="error">{error}</p>{/if}

{#snippet choice(event: SoundEvent, ref: SoundRef | null, text: string, sub?: string)}
  {@const on = picked(event, ref)}
  <!-- A real radio, so arrow keys move through the list and it is one Tab stop. -->
  <label class="nav-row choice">
    <input type="radio" class="sr-only" name={`sound-${event}`} checked={on} onchange={() => choose(event, ref)} />
    <span class="grow">{text}{#if sub}<span class="sub">{sub}</span>{/if}</span>
    <span class="check" aria-hidden="true">{#if on}<Icon name="check" size={16} />{/if}</span>
  </label>
{/snippet}

<style>
  .gap-top { margin-top: 18px; }
  input[type=range] { accent-color: var(--accent); width: 150px; min-width: 65px; } output { width: 38px; font-size: var(--fs-xs); text-align: right; font-variant-numeric: tabular-nums; color: var(--ink-2); }
  select.field { width: auto; min-width: 0; max-width: 100%; }
  /* A file input inside a row: the row is the button. */
  .file { position: relative; display: block; min-height: 48px; padding: 12px 16px; color: var(--lamp); font-weight: 600; cursor: pointer; }
  .file input { position: absolute; inset: 0; opacity: 0; cursor: pointer; width: 100%; }
  :global(:where(:root:not([data-input='touch']))) .file:focus-within { outline: 2px solid var(--lamp); outline-offset: -3px; }
  .file:active { background: var(--press); }
  .sub { display: block; font-size: var(--fs-xs); color: var(--ink-3); }
  .check { display: grid; width: 18px; color: var(--lamp); }
  .choice { position: relative; cursor: pointer; }
  :global(:where(:root:not([data-input='touch']))) .choice:has(input:focus-visible) { outline: 2px solid var(--lamp); outline-offset: -3px; }
  .feedback { font-size: var(--fs-sm); color: var(--success); padding: 10px 4px 0; } .error { color: var(--danger); }
  @media (max-width: 650px) { input[type=range] { flex: 1; width: auto; min-width: 120px; } }
  @media (pointer: coarse) { .file { min-height: 52px; padding-block: 14px; } }
</style>
