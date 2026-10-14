<script lang="ts">
  // Admins name the server and look after its rooms: add, group, give an icon, delete.
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import { themes } from '../lib/theme.svelte'
  import { roomName } from '../lib/room-list'
  import type { Category, Channel } from '../lib/types'
  import IconPicker from './IconPicker.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import SettingRow from './SettingRow.svelte'

  let serverName = $state(''), nameBusy = $state(false), nameStatus = $state('')
  let newCat = $state(''), newChan = $state(''), newChanCat = $state(''), error = $state('')
  $effect(() => { serverName = store.settings.instance_name })
  async function saveName(e: SubmitEvent) {
    e.preventDefault(); nameBusy = true; nameStatus = ''; error = ''
    try {
      store.settings = await api.put('/settings', { instance_name: serverName.trim() })
      nameStatus = 'Server name saved.'
    } catch (err) { error = (err as Error).message }
    finally { nameBusy = false }
  }
  async function addCategory(e: SubmitEvent) {
    e.preventDefault(); error = ''
    try { await api.post<Category>('/categories', { name: newCat.trim(), position: store.categories.length }); newCat = ''; await store.resync() } catch (err) { error = (err as Error).message }
  }
  async function addChannel(e: SubmitEvent) {
    e.preventDefault(); error = ''
    try { await api.post<Channel>('/channels', { name: newChan.trim().toLowerCase(), category_id: newChanCat || null, position: store.textChannels.length }); newChan = ''; await store.resync() } catch (err) { error = (err as Error).message }
  }
  async function delChannel(c: Channel) { await api.del(`/channels/${c.id}`); await store.resync() }
  async function delCategory(c: Category) { await api.del(`/categories/${c.id}`); await store.resync() }
  const name = (c: Channel) => roomName(c, themes.appearance.room_icons)
</script>

<h2 class="display">Rooms</h2>
<p class="muted">Name the server, add rooms and categories, and give each room an icon.</p>

<h3 class="list-head">Server</h3>
<form class="slist" onsubmit={saveName}>
  <SettingRow find="server-name" label="Server name" hint="1–40 characters. Shown to members and on the login screen." wide>
    {#snippet control()}
      <div class="inline">
        <input id="server-name" class="field" bind:value={serverName} required aria-label="Server name" />
        <button class="btn" type="submit" disabled={nameBusy || !serverName.trim() || Array.from(serverName.trim()).length > 40}>Save name</button>
      </div>
    {/snippet}
  </SettingRow>
</form>
<p class="list-foot" role="status">{nameStatus}</p>

<h3 class="list-head">Add</h3>
<div class="slist">
  <form onsubmit={addChannel}>
    <SettingRow find="add-room" label="New room" hint="Lowercase letters, numbers, - and _." wide>
      {#snippet control()}
        <div class="inline">
          <input class="field" bind:value={newChan} placeholder="new-room" aria-label="New room name" pattern={'[a-z0-9_-]{1,40}'} required />
          <select class="field" bind:value={newChanCat} aria-label="Category for the new room">
            <option value="">No category</option>
            {#each store.categories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
          </select>
          <button class="btn lit" type="submit">Add room</button>
        </div>
      {/snippet}
    </SettingRow>
  </form>
  <form onsubmit={addCategory}>
    <SettingRow find="add-category" label="New category" hint="Groups rooms together in the room list." wide>
      {#snippet control()}
        <div class="inline">
          <input class="field" bind:value={newCat} placeholder="Games" aria-label="New category name" required />
          <button class="btn" type="submit">Add category</button>
        </div>
      {/snippet}
    </SettingRow>
  </form>
</div>
{#if error}<p class="error" role="alert">{error}</p>{/if}

<div data-find="rooms">
  {#each [null, ...store.categories] as cat (cat?.id ?? 'none')}
    {@const chans = store.textChannels.filter((c) => (c.category_id || null) === (cat?.id || null))}
    {#if cat || chans.length}
      <h3 class="list-head">
        <span>{cat ? cat.name : 'No category'}</span>
        {#if cat}<span class="cat-action"><InlineConfirm action="Delete category" sentence={`Delete the "${cat.name}" category? Its rooms stay, uncategorised.`} confirm={() => delCategory(cat)} /></span>{/if}
      </h3>
      {#if chans.length}
        <div class="slist">
          {#each chans as c (c.id)}
            <div class="room">
              <IconPicker channel={c} />
              <span class="name">{name(c)}</span>
              <InlineConfirm action="Delete" sentence={`Delete ${name(c)} and every message in it?`} confirm={() => delChannel(c)} />
            </div>
          {/each}
        </div>
      {:else}<p class="list-foot">No rooms in this category.</p>{/if}
    {/if}
  {/each}
</div>
<p class="list-foot">Tap a room's icon to choose another. Everyone sees it, unless they picked # signs or nothing in Appearance.</p>

<style>
  .inline { display: flex; flex-wrap: wrap; gap: 8px; width: 100%; }
  .inline .field { flex: 1 1 160px; width: auto; min-width: 0; }
  select.field { appearance: auto; }
  .room { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; min-height: 56px; padding: 6px 12px; }
  .room > :global(.confirmation) { margin-inline-start: auto; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* The category's delete sits on its header line, in normal case. */
  .cat-action { text-transform: none; letter-spacing: 0; font-weight: 400; }
  .cat-action :global(.btn) { min-height: 28px; padding: 2px 8px; font-size: var(--fs-xs); }
  .error { color: var(--danger); }
</style>
