<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import type { Channel } from '../lib/types'

  let { channel }: { channel: Channel } = $props()
  const members = $derived.by(() => {
    const all = [...store.users.values()]
    const list = channel.kind === 'dm' ? all.filter((u) => channel.member_ids?.includes(u.id)) : all
    return list.sort((a, b) => Number(store.online.has(b.id)) - Number(store.online.has(a.id)) || Number(a.bot) - Number(b.bot) || a.username.localeCompare(b.username))
  })
  const onlineCount = $derived(members.filter((u) => store.online.has(u.id)).length)

  async function dm(id: string) {
    if (id === store.me?.id) return
    const c = await store.openDm([id])
    router.go(`/c/${c.id}`)
  }
</script>

<div class="members">
  <div class="eyebrow head">{onlineCount} here · {members.length} total</div>
  {#each members as u (u.id)}
    <button class="person" class:off={!store.online.has(u.id)} onclick={() => dm(u.id)} title={u.id === store.me?.id ? 'You' : `Message ${u.display_name || u.username}`}>
      <Avatar userId={u.id} size={28} />
      <span class="name">{u.display_name || u.username}</span>
      {#if u.bot}<span class="badge" title="Agent"><Icon name="bot" size={12} /></span>{/if}
      {#if u.role === 'admin'}<span class="faint mono tiny">admin</span>{/if}
    </button>
  {/each}
</div>

<style>
  .members { padding: 12px 8px; }
  .head { padding: 4px 10px 10px; }
  .person {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 5px 10px;
    border-radius: var(--r); text-align: left; color: var(--ink);
  }
  .person:hover { background: var(--bg-3); }
  .person.off { color: var(--ink-3); }
  .person.off :global(.avatar) { opacity: 0.55; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .badge { color: var(--lamp); display: grid; }
  .tiny { font-size: 10px; }
</style>
