<script lang="ts">
  // The phone's Appearance page has no room behind it to show changes on, so it
  // carries a small one: two rooms from the list, a person, a few messages, drawn
  // with the live theme, accent, corners, density, icons, bubbles and wallpaper.
  // It stays pinned under the top bar, so every change below shows as it is made.
  import { store } from '../lib/store.svelte'
  import { themes } from '../lib/theme.svelte'
  import { roomName } from '../lib/room-list'
  import Avatar from './Avatar.svelte'
  import BubblePreview from './BubblePreview.svelte'
  import RoomIcon from './RoomIcon.svelte'

  const rooms = $derived(store.textChannels.filter((c) => c.kind === 'text').slice(0, 2))
  const friend = $derived(store.people.find((u) => !u.bot && u.id !== store.me?.id))
</script>

<div class="preview-pin">
  <div class="frame bg-host scope-app" aria-label="Preview of your choices" role="img">
    <div class="rows bg-host scope-sidebar">
      {#each rooms as c, i (c.id)}
        <div class="row">
          <RoomIcon channel={c} size={28} />
          <b>{roomName(c, themes.appearance.room_icons)}</b>
          {#if i === 0}<span class="count">2</span>{/if}
        </div>
      {/each}
      {#if friend}
        <div class="row">
          <Avatar userId={friend.id} size={28} />
          <b>{friend.display_name || friend.username}</b>
        </div>
      {/if}
    </div>
    <div class="chat bg-host scope-chat"><BubblePreview bare /></div>
  </div>
</div>

<style>
  /* Pinned under the NavBar (44pt plus the status bar). Once the page scrolls it
     takes the bar's material, so the settings slide under one bar, not two. */
  .preview-pin {
    position: sticky; top: calc(var(--nav-inset, 0px) + 44px); z-index: 2;
    margin: -4px -16px 0; padding: 4px 16px 10px;
    transition: background-color var(--t-fast), box-shadow var(--t-fast);
  }
  :global(.pane:has(> .navbar.scrolled)) .preview-pin {
    background: color-mix(in srgb, var(--nav-bg, var(--bg)) 90%, transparent);
    -webkit-backdrop-filter: saturate(1.8) blur(20px); backdrop-filter: saturate(1.8) blur(20px);
    box-shadow: 0 .5px 0 var(--line);
  }
  :global(.pane:has(.preview-pin) > .navbar.scrolled) { box-shadow: none; }
  .frame {
    display: grid; grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
    border-radius: var(--r-lg); overflow: clip; border: 1px solid var(--line); background: var(--bg);
  }
  .rows { display: grid; align-content: center; background: var(--bg-2); padding: 4px 0; border-inline-end: 1px solid var(--line); }
  :global(:root[data-bg-scope='app']) .rows { background: color-mix(in srgb, var(--bg-2) 82%, transparent); }
  .row { display: flex; align-items: center; gap: 8px; min-width: 0; padding: calc(5px * var(--density, 1)) 10px; }
  .row b { flex: 1; min-width: 0; font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { min-width: 18px; height: 18px; padding: 0 5px; display: inline-grid; place-items: center; border-radius: var(--r-pill); background: var(--lamp); color: var(--on-lamp); font-size: 11px; font-weight: 700; }
  .chat { display: grid; align-content: center; min-width: 0; padding: 8px 10px; font-size: var(--fs-xs); }
  /* Three bubbles and a name in the height of three rows: smaller type, one line each. */
  .chat :global(.preview) { gap: 6px; font-size: var(--fs-xs); }
  .chat :global(.b) { max-width: 92%; padding: 4px 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chat :global(.who) { font-size: 11px; }
</style>
