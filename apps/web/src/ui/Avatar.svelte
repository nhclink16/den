<script lang="ts">
  import { store, type Store } from '../lib/store.svelte'
  import { mediaUrl } from '../lib/native'
  import { personHue } from '../lib/people.svelte'
  let { userId, size = 32, instance = store, presence = true }: { userId: string; size?: number; instance?: Store; presence?: boolean } = $props()
  const user = $derived(instance.user(userId))
  const label = $derived(user ? (user.display_name || user.username) : '?')
  const online = $derived(instance.online.has(userId))
  // Everyone gets a colour of their own, so a glance at the gutter says who is talking
  // before the name does.
  const hue = $derived(personHue(userId, user))
</script>

<span class="avatar" class:online={online && presence} style="--s:{size}px; --hue:{hue}" title={label}>
  {#if user?.avatar_url}
    <img src={mediaUrl(user.avatar_url, instance.origin)} alt="" />
  {:else}
    <span class="initial" aria-hidden="true">{label.slice(0, 1).toUpperCase()}</span>
  {/if}
  {#if presence}<span class="sr-only" role="img" aria-label={online ? 'Online' : 'Offline'}></span>{/if}
  {#if online && presence}<span class="dot" aria-hidden="true"></span>{/if}
</span>

<style>
  .avatar {
    position: relative; display: inline-grid; place-items: center; flex: none;
    width: var(--s); height: var(--s); border-radius: var(--avatar-r, 35%);
    /* Tinted from the person's hue but mixed into the theme's own raised surface,
       so a warm theme keeps warm avatars and a cool one cool ones. */
    background: color-mix(in oklch, oklch(var(--tint-bg-l, .4) .09 var(--hue)) 60%, var(--bg3));
    color: oklch(var(--tint-ink-l, .88) .11 var(--hue));
    font-family: var(--display); font-weight: 700; font-size: calc(var(--s) * 0.5);
    overflow: visible;
  }
  img { width: 100%; height: 100%; border-radius: var(--avatar-r, 35%); object-fit: cover; }
  /* Online, three ways, chosen per device. The dot is small and quiet: success green
     rather than the accent, so it never reads as an unread badge. */
  .dot {
    position: absolute; right: -1px; bottom: -1px; border-radius: 50%;
    width: max(7px, calc(var(--s) * .24)); height: max(7px, calc(var(--s) * .24));
    background: var(--success); box-shadow: 0 0 0 2px var(--presence-cut, var(--bg-2));
  }
  :global(:root[data-presence='ring']) .dot, :global(:root[data-presence='off']) .dot { display: none; }
  /* A thin ring with a gap, drawn around the shape itself so it follows the corners. */
  :global(:root[data-presence='ring']) .avatar.online {
    box-shadow: 0 0 0 2px var(--presence-cut, var(--bg-2)), 0 0 0 3.5px var(--success);
  }
</style>
