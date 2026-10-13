<script lang="ts">
  // What sits before a room's name: its Lucide icon in a round tile, a #, or
  // nothing, as each person chose in Appearance. A DM always shows the person.
  // `bare` drops the tile (room headers); `name` draws that icon whatever the
  // choice (the picker).
  import type { Channel } from '../lib/types'
  import { store } from '../lib/store.svelte'
  import { themes } from '../lib/theme.svelte'
  import { defaultIcon, iconUrl, roomIcon } from '../lib/room-list'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'

  let { channel, size = 34, bare = false, name }: { channel?: Pick<Channel, 'name' | 'kind' | 'icon' | 'member_ids'>; size?: number; bare?: boolean; name?: string } = $props()
  const mode = $derived(name ? 'icons' : themes.appearance.room_icons ?? 'icons')
  // A name this build of Lucide doesn't have (renamed upstream, or set through
  // the API) falls back to the room's default instead of an empty circle.
  let missing = $state('')
  const icon = $derived.by(() => { const n = name ?? (channel ? roomIcon(channel) : 'message-circle'); return n === missing && channel ? defaultIcon(channel) : n })
  const other = $derived(channel?.kind === 'dm' ? (channel.member_ids || []).find((id) => id !== store.me?.id) || store.me?.id || '' : '')
</script>

{#if other && !bare}
  <Avatar userId={other} {size} />
{:else if mode !== 'none' && (name || channel?.kind !== 'dm')}
  <span class="tile" class:bare style:--size="{size}px" aria-hidden="true">
    {#if mode === 'hash' && channel?.kind === 'text'}<Icon name="hash" size={Math.round(bare ? size : size * 0.5)} />
    {:else}<span class="glyph" style:--icon="url({iconUrl(icon)})"></span><img src={iconUrl(icon)} alt="" hidden onerror={() => (missing = icon)} />{/if}
  </span>
{/if}

<style>
  .tile {
    width: var(--size); height: var(--size); flex: none; display: grid; place-items: center;
    border-radius: var(--avatar-r, 50%); background: var(--bg-3); color: var(--ink-2);
  }
  .tile.bare { width: auto; height: auto; background: none; color: inherit; }
  /* The file is drawn as a mask so the icon takes the text colour. */
  .glyph {
    width: calc(var(--size) * 0.5); height: calc(var(--size) * 0.5); background: currentColor;
    -webkit-mask: var(--icon) center / contain no-repeat; mask: var(--icon) center / contain no-repeat;
  }
  .bare .glyph { width: var(--size); height: var(--size); }
</style>
