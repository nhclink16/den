<script lang="ts">
  import { call } from '../lib/call.svelte'
  import ParticipantVolume from './ParticipantVolume.svelte'
  import { store, instances } from '../lib/store.svelte'
  import { profileCard } from '../lib/people.svelte'
  import { shownActivities, activityVerb, activityIcon } from '../lib/activity'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import type { Channel } from '../lib/types'

  let { channel }: { channel: Channel } = $props()
  const members = $derived.by(() => {
    const all = store.people
    const list = channel.kind === 'dm' ? all.filter((u) => channel.member_ids?.includes(u.id)) : all
    return list.sort((a, b) => Number(store.online.has(b.id)) - Number(store.online.has(a.id)) || Number(a.bot) - Number(b.bot) || a.username.localeCompare(b.username))
  })
  const here = $derived(members.filter((u) => store.online.has(u.id)))
  const away = $derived(members.filter((u) => !store.online.has(u.id)))
  // Whose volume control is open under their row: one at a time, by their call identity.
  let audio = $state('')
  const closeAudio = (e: KeyboardEvent) => {
    if (e.key !== 'Escape' || !audio) return
    e.preventDefault()
    const row = (e.currentTarget as HTMLElement)
    audio = ''
    row.querySelector<HTMLElement>('.speaker')?.focus()
  }

</script>

<div class="roster">
  <div class="head"><span class="eyebrow">People</span><span class="total">{members.length}</span></div>
  <!-- In the call: a speaker at the end of the row, which opens their volume under it. Sound-off when you muted them. -->
  {#snippet speaker(key: string, userId: string, name: string)}
    <button class="speaker" aria-expanded={audio === key} aria-label={`${name} audio options`} title="In call · Volume" onclick={() => (audio = audio === key ? '' : key)}><Icon name={call.volume(userId) === 0 ? 'sound-off' : 'sound'} size={16} /></button>
    {#if audio === key}<div class="volume"><ParticipantVolume {userId} {name} /></div>{/if}
  {/snippet}
  {#each call.origin === store.origin ? call.participants.filter(p => p.music) : [] as dj (dj.id)}
    <!-- svelte-ignore a11y_no_static_element_interactions (Escape closes the volume control) -->
    <div class="member member-audio" onkeydown={closeAudio}><div class="person"><Icon name="music" size={28} /><span class="name">{dj.name}</span><span class="badge">DJ</span></div>{@render speaker(`dj:${dj.id}`, dj.userId, dj.name)}</div>
  {/each}
  {#snippet person(u: import('../lib/types').User)}
    {@const doing = shownActivities(store.activities.get(u.id))[0]}
    {@const inCall = call.origin === store.origin && call.participants.some(p => p.userId === u.id && !p.local)}
    <!-- svelte-ignore a11y_no_static_element_interactions (Escape closes the volume control) -->
    <div class="member" class:member-audio={inCall} onkeydown={inCall ? closeAudio : undefined}>
    <button class="person" data-hover-animates class:off={!store.online.has(u.id)} aria-haspopup="dialog" onclick={(e) => profileCard.open(u.id, e.currentTarget, instances.active)}>
      <Avatar userId={u.id} size={28} />
      <span class="who">
        <span class="top">
          <span class="name">{u.display_name || u.username}</span>
          {#if u.bot}<span class="tag lit"><Icon name="bot" size={11} />Agent</span>{/if}
          {#if u.role === 'admin'}<span class="tag">Admin</span>{/if}
        </span>
        {#if doing}<span class="doing" class:live={doing.kind === 'playing'}><Icon name={activityIcon[doing.kind]} size={11} /><span>{activityVerb(doing.kind)} <b>{doing.name}</b></span></span>{/if}
      </span>
    </button>
    {#if inCall}{@render speaker(u.id, u.id, u.display_name || u.username)}{/if}
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
  /* The person, then their speaker; the volume control spans the row underneath. */
  .member { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; }
  .speaker {
    width: 24px; height: 24px; margin-inline: 4px 6px; display: grid; place-items: center; border-radius: var(--r-pill); color: var(--ink-2);
    transition: background-color var(--t-fast), color var(--t-fast);
  }
  @media (hover: hover) { .speaker:hover { background: var(--hover); color: var(--ink); } }
  .speaker[aria-expanded='true'] { background: var(--press); color: var(--ink); }
  .volume { grid-column: 1 / -1; margin: 2px 10px 8px 48px; }
  .volume :global(.volume-control) { width: 100%; padding-inline: 0; }
  .roster { padding: 0 8px 12px; }
  /* Same height and rule as the room header, so the app's main line runs edge to edge. */
  .head {
    display: flex; align-items: center; justify-content: space-between; min-height: 52px;
    margin: 0 -8px 4px; padding: 0 18px; border-bottom: 1px solid var(--line);
  }
  .total { font-size: var(--fs-xs); color: var(--ink-2); padding: 1px 7px; border-radius: var(--r-pill, 999px); background: var(--bg-3); }
  .group { display: flex; gap: 6px; margin: 14px 10px 6px; font-weight: 600; color: var(--ink-2); }
  .group span { color: var(--ink-3); }
  .person {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 5px 10px;
    border-radius: var(--r); text-align: left; color: var(--ink);
    transition: background-color var(--t-fast), color var(--t-fast);
  }
  @media (hover: hover) { .person:hover { background: color-mix(in srgb, var(--ink) 6%, transparent); } }
  .person.off { color: var(--ink-2); }
  .person.off :global(.avatar) { opacity: 0.55; filter: saturate(.4); transition: opacity var(--t), filter var(--t); }
  @media (hover: hover) { .person.off:hover :global(.avatar) { opacity: 1; filter: none; } }
  .who { flex: 1; min-width: 0; display: grid; }
  .top { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-body); }
  /* What they are up to, under the name. A game is live, so it is lit. */
  .doing { display: flex; align-items: center; gap: 4px; min-width: 0; font-size: var(--fs-xs); color: var(--ink-2); }
  .doing > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .doing b { font-weight: 600; color: var(--ink); }
  .doing :global(svg) { flex: none; }
  .doing.live, .doing.live b { color: var(--lamp); }
  .badge { color: var(--lamp); display: grid; }
  .tag {
    display: inline-flex; align-items: center; gap: 3px; flex: none; padding: 1px 6px; border-radius: var(--r-pill, 999px);
    font: 600 var(--fs-xs)/1.45 var(--body);
    color: var(--ink-2); background: var(--bg-3);
  }
  .tag.lit { color: var(--lamp); background: var(--lamp-glow); }
</style>
