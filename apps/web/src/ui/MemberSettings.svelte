<script lang="ts">
  // Admins look after the member list: names, pictures, and removing someone.
  // A removed member is signed out for good; their messages stay readable.
  import { store } from '../lib/store.svelte'
  import type { User } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import InlineConfirm from './InlineConfirm.svelte'

  const people = $derived(store.people.filter((u) => !u.bot).sort((a, b) => a.username.localeCompare(b.username)))
  const removed = $derived([...store.users.values()].filter((u) => u.removed))
  let editing = $state<string | null>(null)
  // One person's actions show at a time, under their row, so a phone never stacks six buttons per person.
  let shown = $state<string | null>(null)
  let username = $state(''), display = $state('')
  let busy = $state<string | null>(null)
  let error = $state(''), note = $state('')

  function edit(u: User) { editing = u.id; username = u.username; display = u.display_name === u.username ? '' : u.display_name; error = '' }
  async function run(id: string, done: string, work: () => Promise<void>) {
    busy = id; error = ''; note = ''
    try { await work(); note = done } catch (e) { error = e instanceof Error ? e.message : 'That did not work.' } finally { busy = null }
  }
  function rename(u: User, e: SubmitEvent) {
    e.preventDefault()
    void run(u.id, 'Saved.', async () => { await store.renameMember(u.id, { username: username.trim(), display_name: display.trim() }); editing = null })
  }

  /** A GIF goes up as is so it keeps moving; anything else is cropped to its centre. */
  async function square(file: File): Promise<Blob> {
    if (file.type === 'image/gif') return file
    const url = URL.createObjectURL(file)
    try {
      const img = await new Promise<HTMLImageElement>((ok, fail) => { const i = new Image(); i.onload = () => ok(i); i.onerror = fail; i.src = url })
      const w = img.naturalWidth, h = img.naturalHeight
      const side = Math.min(w, h), size = Math.min(side, 1024), canvas = document.createElement('canvas')
      canvas.width = canvas.height = size
      canvas.getContext('2d')!.drawImage(img, (w - side) / 2, (h - side) / 2, side, side, 0, 0, size, size)
      return await new Promise<Blob>((ok, fail) => canvas.toBlob((b) => (b ? ok(b) : fail(new Error('That picture could not be read.'))), 'image/png'))
    } finally { URL.revokeObjectURL(url) }
  }
  function picture(u: User, kind: 'avatar' | 'banner', input: HTMLInputElement) {
    const file = input.files?.[0]; input.value = ''
    if (!file) return
    void run(u.id, kind === 'avatar' ? 'Picture updated.' : 'Banner updated.', async () => {
      await store.setProfileImage(kind, kind === 'avatar' ? await square(file) : file, u.id)
    })
  }
  const accept = 'image/png,image/jpeg,image/webp,image/gif'
</script>

<h2 class="display">Members</h2>
<p class="muted">Fix a name, set someone's picture, or remove someone. Removing signs them out everywhere and stops them logging in; their messages stay.</p>
<p class="status" class:error={!!error} role="status">{error || note}</p>

<div class="slist" data-find="people">
  {#each people as u (u.id)}
    {@const me = u.id === store.me?.id}
    {@const open = shown === u.id || editing === u.id}
    <div class="person" data-hover-animates class:busy={busy === u.id}>
      <button class="who-row" aria-expanded={open} onclick={() => { shown = open ? null : u.id; editing = null }}>
        <Avatar userId={u.id} size={40} presence={false} />
        <span class="who"><b>{u.display_name || u.username}</b><span class="faint">@{u.username}{#if u.role === 'admin'}{' · admin'}{/if}{#if me}{' · you'}{/if}</span></span>
        <span class="chev" class:up={open}><Icon name="chevron" size={14} /></span>
      </button>
      {#if editing === u.id}
        <form class="rename" onsubmit={(e) => rename(u, e)}>
          <label><span class="eyebrow">Username</span><input class="field" bind:value={username} required minlength="3" maxlength="32" autocomplete="off" /></label>
          <label><span class="eyebrow">Display name</span><input class="field" bind:value={display} placeholder={username} maxlength="100" autocomplete="off" /></label>
          <div class="row">
            <button class="btn lit" type="submit">Save</button>
            <button class="btn quiet" type="button" onclick={() => (editing = null)}>Cancel</button>
          </div>
        </form>
      {:else if open}
        <div class="actions">
          <button class="btn" onclick={() => edit(u)}><Icon name="edit" size={14} /> Rename</button>
          <label class="btn"><Icon name="plus" size={14} /> Picture<input class="sr-only" type="file" {accept} onchange={(e) => picture(u, 'avatar', e.currentTarget)} /></label>
          <label class="btn"><Icon name="plus" size={14} /> Banner<input class="sr-only" type="file" {accept} onchange={(e) => picture(u, 'banner', e.currentTarget)} /></label>
          {#if u.avatar_url}<button class="btn quiet" onclick={() => run(u.id, 'Picture cleared.', () => store.setProfileImage('avatar', null, u.id))}>Clear picture</button>{/if}
          {#if u.banner_url}<button class="btn quiet" onclick={() => run(u.id, 'Banner cleared.', () => store.setProfileImage('banner', null, u.id))}>Clear banner</button>{/if}
          {#if !me}
            <InlineConfirm action="Remove" sentence={`Remove ${u.display_name || u.username}? They're signed out everywhere and can't log in again. Their messages stay.`} confirm={() => run(u.id, 'Removed.', () => store.removeMember(u.id))} />
          {/if}
        </div>
      {/if}
    </div>
  {/each}
</div>
<p class="list-foot">Tap someone to rename them, set their picture, or remove them.</p>

{#if removed.length}
  <h3 class="list-head">Removed</h3>
  <p class="list-foot">{removed.map((u) => u.display_name || u.username).join(', ')}</p>
{/if}

<style>
  .person.busy { opacity: .6; pointer-events: none; }
  .who-row { display: flex; align-items: center; gap: 12px; width: 100%; min-height: 60px; padding: 8px 16px; text-align: start; transition: background-color var(--t-fast); }
  @media (hover: hover) { .who-row:hover { background-color: var(--hover); } }
  .who { display: grid; min-width: 0; flex: 1; line-height: 1.3; }
  .who b { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who .faint { font-size: var(--fs-xs); }
  .chev { display: grid; color: var(--ink-3); transform: rotate(-90deg); transition: transform var(--t-fast); }
  .chev.up { transform: none; }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; padding: 0 16px 14px 68px; }
  .actions label { cursor: pointer; }
  .actions label:focus-within { outline: 2px solid var(--lamp); outline-offset: 2px; }
  .rename { display: flex; flex-wrap: wrap; gap: 8px; align-items: end; padding: 0 16px 14px 68px; }
  .rename label { display: grid; gap: 4px; flex: 1 1 160px; }
  .row { display: flex; gap: 6px; }
  .status { min-height: 1.4em; margin: 8px 0 0; font-size: var(--fs-sm); color: var(--ink-2); }
  .status.error { color: var(--danger); }
  @media (max-width: 650px) { .actions, .rename { padding-left: 16px; } }
</style>
