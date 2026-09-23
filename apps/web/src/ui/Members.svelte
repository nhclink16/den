<script lang="ts">
  import { call } from '../lib/call.svelte'
  import ParticipantVolume from './ParticipantVolume.svelte'
  import { store, instances } from '../lib/store.svelte'
  import { profileCard } from '../lib/people.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import type { Channel } from '../lib/types'

  let { channel }: { channel: Channel } = $props()
  const members = $derived.by(() => {
    const all = [...store.users.values()]
    const list = channel.kind === 'dm' ? all.filter((u) => channel.member_ids?.includes(u.id)) : all
    return list.sort((a, b) => Number(store.online.has(b.id)) - Number(store.online.has(a.id)) || Number(a.bot) - Number(b.bot) || a.username.localeCompare(b.username))
  })
  const here = $derived(members.filter((u) => store.online.has(u.id)))
  const away = $derived(members.filter((u) => !store.online.has(u.id)))

</script>

<div class="roster">
  <div class="head"><span class="eyebrow">People</span><span class="total mono">{members.length}</span></div>
  {#each call.origin === store.origin ? call.participants.filter(p => p.music) : [] as dj (dj.id)}
    <div class="member"><div class="person"><Icon name="music" size={28} /><span class="name">{dj.name}</span><span class="badge">DJ</span></div><details class="member-audio"><summary aria-label={`${dj.name} audio options`}><Icon name="sound" size={14} /><span>In call</span></summary><ParticipantVolume userId={dj.userId} name={dj.name} /></details></div>
  {/each}
  {#snippet person(u: import('../lib/types').User)}
    <div class="member">
    <button class="person" class:off={!store.online.has(u.id)} aria-haspopup="dialog" onclick={(e) => profileCard.open(u.id, e.currentTarget, instances.active)}>
      <Avatar userId={u.id} size={28} />
      <span class="name">{u.display_name || u.username}</span>
      {#if u.bot}<span class="tag lit"><Icon name="bot" size={11} />agent</span>{/if}
      {#if u.role === 'admin'}<span class="tag">admin</span>{/if}
    </button>
    {#if call.origin === store.origin && call.participants.some(p => p.userId === u.id && !p.local)}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
      <details class="member-audio" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
        <summary aria-label={`${u.display_name || u.username} audio options`}><Icon name={call.volume(u.id) === 0 ? 'sound-off' : 'sound'} size={14} /><span>In call</span></summary>
        <ParticipantVolume userId={u.id} name={u.display_name || u.username} />
      </details>
    {/if}
    </div>
  {/snippet}
  {#if here.length}
    <h2 class="group eyebrow">Here <span>{here.length}</span></h2>
    {#each here as u (u.id)}{@render person(u)}{/each}
  {/if}
  {#if away.length}
    <h2 class="group eyebrow">Away <span>{away.length}</span></h2>
    {#each away as u (u.id)}{@render person(u)}{/each}
  {/if}
</div>

<style>
  .member-audio { margin: 0 10px 8px 48px; }
  summary { display: flex; align-items: center; gap: 6px; min-height: 24px; cursor: pointer; color: var(--ink-2); font-size: 11px; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  .member-audio :global(.volume-control) { width: 100%; padding-inline: 0; }
  .roster { padding: 0 8px 12px; }
  /* Same height and rule as the room header, so the app's main line runs edge to edge. */
  .head {
    display: flex; align-items: center; justify-content: space-between; min-height: 52px;
    margin: 0 -8px 4px; padding: 0 18px; border-bottom: 1px solid var(--line);
  }
  .total { font-size: 11px; color: var(--ink-2); padding: 1px 7px; border-radius: var(--r-pill, 999px); background: var(--bg-3); }
  .group { display: flex; gap: 6px; margin: 14px 10px 6px; font-weight: 600; color: var(--ink-2); }
  .group span { color: var(--ink-3); }
  .person {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 5px 10px;
    border-radius: var(--r); text-align: left; color: var(--ink);
    transition: background-color var(--t-fast), color var(--t-fast);
  }
  .person:hover { background: color-mix(in srgb, var(--ink) 6%, transparent); }
  .person.off { color: var(--ink-2); }
  .person.off :global(.avatar) { opacity: 0.55; filter: saturate(.4); transition: opacity var(--t), filter var(--t); }
  .person.off:hover :global(.avatar) { opacity: 1; filter: none; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .badge { color: var(--lamp); display: grid; }
  .tag {
    display: inline-flex; align-items: center; gap: 3px; flex: none; padding: 1px 6px; border-radius: var(--r-pill, 999px);
    font: 600 11px/1.45 var(--mono); letter-spacing: .04em; text-transform: uppercase;
    color: var(--ink-2); background: var(--bg-3);
  }
  .tag.lit { color: var(--lamp); background: var(--lamp-glow); }
</style>
