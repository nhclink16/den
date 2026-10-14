<script lang="ts">
  // The phone's Appearance page has no room behind it to show changes on, so it
  // carries a small one: two rooms from the list, a person, a few messages and
  // the message box, drawn with the live theme, accent, corners, icons, bubbles
  // and wallpaper.
  import { store } from '../lib/store.svelte'
  import { themes } from '../lib/theme.svelte'
  import { roomName } from '../lib/room-list'
  import Avatar from './Avatar.svelte'
  import BubblePreview from './BubblePreview.svelte'
  import RoomIcon from './RoomIcon.svelte'

  const rooms = $derived(store.textChannels.filter((c) => c.kind === 'text').slice(0, 2))
  const friend = $derived(store.people.find((u) => !u.bot && u.id !== store.me?.id))
  const lines = ['Maya: are we still on for 9?', 'Theo: who has the key?']
</script>

<div class="frame bg-host scope-app" aria-label="Preview of your choices" role="img">
  <div class="rows bg-host scope-sidebar">
    {#each rooms as c, i (c.id)}
      <div class="row">
        <RoomIcon channel={c} size={36} />
        <span class="mid"><span class="top"><b>{roomName(c, themes.appearance.room_icons)}</b><span class="tm">9:4{i + 1}</span></span><span class="pv">{lines[i]}</span></span>
        {#if i === 0}<span class="count">2</span>{/if}
      </div>
    {/each}
    {#if friend}
      <div class="row">
        <Avatar userId={friend.id} size={36} />
        <span class="mid"><span class="top"><b>{friend.display_name || friend.username}</b><span class="tm">Here</span></span><span class="pv">Online now</span></span>
      </div>
    {/if}
  </div>
  <div class="chat bg-host scope-chat">
    <BubblePreview bare />
    <div class="box"><span>Message {rooms[0] ? roomName(rooms[0], themes.appearance.room_icons) : ''}</span><span class="send"></span></div>
  </div>
</div>

<style>
  .frame { display: grid; border-radius: var(--r-lg); overflow: clip; border: 1px solid var(--line); background: var(--bg); margin-bottom: 8px; }
  .rows { background: var(--bg-2); padding: 4px 0; }
  :global(:root[data-bg-scope='app']) .rows { background: color-mix(in srgb, var(--bg-2) 82%, transparent); }
  .row { display: flex; align-items: center; gap: 10px; padding: calc(6px * var(--density, 1)) 12px; }
  .mid { flex: 1; min-width: 0; display: grid; line-height: 1.3; }
  .top { display: flex; align-items: baseline; gap: 6px; }
  .top b { flex: 1; min-width: 0; font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tm { font-size: var(--fs-xs); color: var(--ink-3); }
  .pv { font-size: var(--fs-xs); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { min-width: 20px; height: 20px; padding: 0 6px; display: inline-grid; place-items: center; border-radius: var(--r-pill); background: var(--lamp); color: var(--on-lamp); font-size: 12px; font-weight: 700; }
  .chat { display: grid; gap: 10px; padding: 12px; border-top: 1px solid var(--line); }
  .box { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 36px; padding: 4px 6px 4px 14px; border-radius: var(--r-pill); background: var(--bg-3); color: var(--ink-3); font-size: var(--fs-sm); }
  .send { width: 26px; height: 26px; border-radius: 50%; background: var(--lamp); flex: none; }
</style>
