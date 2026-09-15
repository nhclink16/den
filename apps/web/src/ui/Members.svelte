<script lang="ts">
  import { call } from '../lib/call.svelte'
  import ParticipantVolume from './ParticipantVolume.svelte'
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

<div class="roster">
  <div class="eyebrow head">{onlineCount} here · {members.length} total</div>
  {#each call.origin === store.origin ? call.participants.filter(p => p.music) : [] as dj (dj.id)}
    <div class="member"><div class="person"><Icon name="music" size={28} /><span class="name">{dj.name}</span><span class="badge">DJ</span></div><details class="member-audio"><summary aria-label={`${dj.name} audio options`}><Icon name="sound" size={14} /><span>In call</span></summary><ParticipantVolume userId={dj.userId} name={dj.name} /></details></div>
  {/each}
  {#each members as u (u.id)}
    <div class="member">
    <button class="person" class:off={!store.online.has(u.id)} onclick={() => dm(u.id)} title={u.id === store.me?.id ? 'You' : `Message ${u.display_name || u.username}`}>
      <Avatar userId={u.id} size={28} />
      <span class="name">{u.display_name || u.username}{#if !store.online.has(u.id)}<span class="presence" aria-hidden="true">Offline</span>{/if}</span>
      {#if u.bot}<span class="badge" title="Agent"><Icon name="bot" size={12} /></span>{/if}
      {#if u.role === 'admin'}<span class="faint mono tiny">admin</span>{/if}
    </button>
    {#if call.origin === store.origin && call.participants.some(p => p.userId === u.id && !p.local)}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
      <details class="member-audio" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
        <summary aria-label={`${u.display_name || u.username} audio options`}><Icon name={call.volume(u.id) === 0 ? 'sound-off' : 'sound'} size={14} /><span>In call</span></summary>
        <ParticipantVolume userId={u.id} name={u.display_name || u.username} />
      </details>
    {/if}
    </div>
  {/each}
</div>

<style>
  .member-audio { margin: 0 10px 8px 48px; }
  summary { display: flex; align-items: center; gap: 6px; min-height: 24px; cursor: pointer; color: var(--ink-2); font-size: 11px; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  .member-audio :global(.volume-control) { width: 100%; padding-inline: 0; }
  .roster { padding: 12px 8px; }
  .head { padding: 4px 10px 10px; }
  .person {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 5px 10px;
    border-radius: var(--r); text-align: left; color: var(--ink);
  }
  .person:hover { background: var(--bg-3); }
  .person.off { color: var(--ink-2); }
  .presence { display: block; font-size: 11px; font-weight: 400; color: var(--ink-2); }
  .person.off :global(.avatar) { opacity: 0.55; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .badge { color: var(--lamp); display: grid; }
  .tiny { font-size: 11px; color: var(--ink-2); }
</style>
