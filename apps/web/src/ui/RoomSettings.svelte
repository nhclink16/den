<script module lang="ts">
  import type { Channel as Room } from '../lib/types'
  // The server takes the whole room, so saves go one at a time, each from the room as the last one left it, and
  // across visits to the page too: a rename still in flight can't be undone by a category change sent with the old
  // name. The server's answer stands in until a resync brings the room back, in case that resync fails.
  let queue = Promise.resolve()
  const answered = new Map<string, Room>()
</script>

<script lang="ts">
  // Admins name the server and look after its rooms. The list comes first, the way iOS lists things: each room
  // and each category opens its own page (`sub`, a room's or category's id) with its name, its place, its icon
  // and a red Delete. New rooms and categories are rows at the top that ask for a name.
  import { store, instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { placedIn } from '../lib/room-list'
  import type { Category, Channel } from '../lib/types'
  import IconPicker from './IconPicker.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import Icon from './Icon.svelte'
  import SettingRow from './SettingRow.svelte'
  import FormDialog from './FormDialog.svelte'

  let { sub }: { sub?: string } = $props()
  // Writes go to the server this page opened on, even if a queued one runs after switching servers.
  const owner = instances.active
  let serverName = $state(''), status = $state(''), error = $state('')
  let newChan = $state(''), newChanCat = $state(''), newCat = $state('')
  let addingRoom = $state(false), addingCat = $state(false), deleting = $state(false)
  $effect(() => { serverName = store.settings.instance_name })
  const room = $derived(sub ? store.textChannels.find((c) => c.id === sub) : undefined)
  const category = $derived(sub && !room ? store.categories.find((c) => c.id === sub) : undefined)

  async function run(work: () => Promise<void>, done = '') {
    error = ''; status = ''
    try { await work(); status = done } catch (err) { error = (err as Error).message }
  }
  function saveName() {
    // Every change saves, even back to the name on the server: an earlier one may still be on its way.
    const name = serverName.trim()
    if (!name) return
    void later(async () => { owner.settings = await owner.api.put('/settings', { instance_name: name }) })
  }
  async function addChannel() {
    await owner.api.post<Channel>('/channels', { name: newChan.trim().toLowerCase(), category_id: newChanCat || null, position: owner.textChannels.length })
    newChan = ''; await owner.resync()
  }
  async function addCategory() {
    await owner.api.post<Category>('/categories', { name: newCat.trim(), position: owner.categories.length })
    newCat = ''; await owner.resync()
  }
  const later = (work: () => Promise<void>) => (queue = queue.then(() => run(work, 'Saved.')))
  const saveRoom = (c: Channel, patch: Partial<Pick<Channel, 'name' | 'category_id'>>) => later(async () => {
    const now = answered.get(c.id) ?? owner.textChannels.find((x) => x.id === c.id) ?? c
    answered.set(c.id, await owner.api.put<Channel>(`/channels/${c.id}`, { name: now.name, category_id: now.category_id ?? null, position: now.position, ...patch }))
    await owner.resync(); answered.delete(c.id)
  })
  const saveCategory = (c: Category, name: string) => later(async () => {
    const now = owner.categories.find((x) => x.id === c.id) ?? c
    await owner.api.put(`/categories/${c.id}`, { name, position: now.position }); await owner.resync()
  })
  /** Off its page and back to the list, then gone. */
  async function remove() {
    await owner.api.del(room ? `/channels/${room.id}` : `/categories/${category!.id}`)
    if (router.parent === '/settings/rooms') router.back(); else router.go('/settings/rooms', true)
    await owner.resync()
  }
  const open = (id: string) => router.go(`/settings/rooms/${id}`, false, '/settings/rooms')
</script>

{#if room}
  <h2 class="display">{room.name}</h2>
  <div class="slist gap-top">
    <SettingRow label="Name">
      {#snippet control()}<input class="field plain value-field" aria-label="Room name" value={room.name} pattern={'[a-z0-9_-]{1,40}'} autocapitalize="off" autocomplete="off" enterkeyhint="done"
        onchange={(e) => { const v = e.currentTarget.value.trim().toLowerCase(); if (v) void saveRoom(room, { name: v }) }} />{/snippet}
    </SettingRow>
    <SettingRow label="Category">
      {#snippet control()}
        <select class="field" aria-label="Category" value={placedIn(room, store.categories) ?? ''} onchange={(e) => saveRoom(room, { category_id: e.currentTarget.value || null })}>
          <option value="">None</option>
          {#each store.categories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
      {/snippet}
    </SettingRow>
    <SettingRow label="Icon">{#snippet control()}<IconPicker channel={room} />{/snippet}</SettingRow>
  </div>
  <p class="list-foot">Lowercase letters, numbers, - and _. Everyone sees the icon, unless they chose # signs or nothing in Appearance.</p>
  <div class="slist gap-top"><button class="action-row danger" onclick={() => (deleting = true)}>Delete Room</button></div>
  <FormDialog bind:open={deleting} title={`Delete ${room.name}?`} action="Delete" danger onsubmit={remove}>
    <p>Every message in it goes too. This can't be undone.</p>
  </FormDialog>
{:else if category}
  <h2 class="display">{category.name}</h2>
  <div class="slist gap-top">
    <SettingRow label="Name">
      {#snippet control()}<input class="field plain value-field" aria-label="Category name" value={category.name} maxlength="40" autocomplete="off" enterkeyhint="done"
        onchange={(e) => { const v = e.currentTarget.value.trim(); if (v) void saveCategory(category, v) }} />{/snippet}
    </SettingRow>
  </div>
  <div class="slist gap-top"><button class="action-row danger" onclick={() => (deleting = true)}>Delete Category</button></div>
  <p class="list-foot">Its rooms stay, without a category.</p>
  <FormDialog bind:open={deleting} title={`Delete ${category.name}?`} action="Delete" danger onsubmit={remove}>
    <p>Its rooms stay, without a category.</p>
  </FormDialog>
{:else}
  <h2 class="display">Rooms</h2>
  <div class="slist gap-top">
    <SettingRow find="server-name" label="Server name">
      {#snippet control()}<input id="server-name" class="field plain value-field" aria-label="Server name" bind:value={serverName} maxlength="40" autocomplete="off" enterkeyhint="done" onchange={saveName} />{/snippet}
    </SettingRow>
  </div>
  <p class="list-foot">Shown to members and on the sign-in screen.</p>
  <div class="slist gap-top">
    <button class="action-row add" data-find="add-room" onclick={() => { newChan = ''; newChanCat = ''; addingRoom = true }}><Icon name="plus" size={15} /> New Room</button>
    <button class="action-row add" data-find="add-category" onclick={() => { newCat = ''; addingCat = true }}><Icon name="plus" size={15} /> New Category</button>
  </div>

  <div data-find="rooms">
    {#each [null, ...store.categories] as cat (cat?.id ?? 'none')}
      {@const chans = store.textChannels.filter((c) => placedIn(c, store.categories) === (cat?.id || null))}
      {#if cat || chans.length}
        <h3 class="list-head">
          {#if cat}<a class="section" href="/settings/rooms/{cat.id}" onclick={(e) => { e.preventDefault(); open(cat.id) }}><span>{cat.name}</span><Icon name="chevron" size={12} /></a>
          {:else}No category{/if}
        </h3>
        {#if chans.length}
          <nav class="slist" aria-label={cat ? cat.name : 'No category'}>
            {#each chans as c (c.id)}
              <a class="nav-row room" href="/settings/rooms/{c.id}" onclick={(e) => { e.preventDefault(); open(c.id) }}>
                <RoomIcon channel={c} size={30} />
                <span class="grow">{c.name}</span>
                <span class="chev"><Icon name="chevron" size={14} /></span>
              </a>
            {/each}
          </nav>
        {:else}<p class="list-foot">No rooms in this category.</p>{/if}
      {/if}
    {/each}
  </div>

  <FormDialog bind:open={addingRoom} title="New Room" action="Add" disabled={!/^[a-z0-9_-]{1,40}$/.test(newChan.trim().toLowerCase())} onsubmit={addChannel}>
    <input class="field" bind:value={newChan} placeholder="new-room" aria-label="Room name" autocapitalize="off" autocomplete="off" />
    <select class="field boxed" bind:value={newChanCat} aria-label="Category for the new room">
      <option value="">No category</option>
      {#each store.categories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
    </select>
    <p>Lowercase letters, numbers, - and _.</p>
  </FormDialog>
  <FormDialog bind:open={addingCat} title="New Category" action="Add" disabled={!newCat.trim()} onsubmit={addCategory}>
    <input class="field" bind:value={newCat} placeholder="Games" aria-label="Category name" autocomplete="off" />
  </FormDialog>
{/if}
{#if error}<p class="error" role="alert">{error}</p>{/if}
<p class="list-foot" role="status">{status}</p>

<style>
  .gap-top { margin-top: 18px; }
  /* A name edited in place: no box, the value in grey on the right, like iOS's text rows. */
  .plain { width: 12em; max-width: 100%; padding: 6px 0; border-color: transparent; background: none; text-align: end; color: var(--ink-2); }
  .plain:focus { color: var(--ink); }
  button.action-row.add { display: flex; align-items: center; gap: 6px; }
  .room { gap: 12px; }
  .slist > .room + .room { background-position: 58px 0; background-size: calc(100% - 58px) 1px; }
  .section { display: inline-flex; align-items: center; gap: 4px; color: inherit; }
  .section :global(svg) { transform: rotate(-90deg); }
  .boxed { appearance: auto; }
  .error { color: var(--danger); }
</style>
