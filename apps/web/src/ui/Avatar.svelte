<script lang="ts">
  import { store, type Store } from '../lib/store.svelte'
  import { mediaUrl } from '../lib/native'
  let { userId, size = 32, instance = store, presence = true }: { userId: string; size?: number; instance?: Store; presence?: boolean } = $props()
  const user = $derived(instance.user(userId))
  const label = $derived(user ? (user.display_name || user.username) : '?')
  const online = $derived(instance.online.has(userId))
  // Everyone gets a stable colour of their own, so a glance at the gutter says who is
  // talking before the name does. Hashing the id keeps it the same on every device.
  const hue = $derived(([...userId].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261) >>> 0) % 360)
</script>

<span class="avatar" style="--s:{size}px; --hue:{hue}" title={label}>
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
    width: var(--s); height: var(--s); border-radius: var(--r-avatar, 35%);
    /* Tinted from the person's hue but mixed into the theme's own raised surface,
       so a warm theme keeps warm avatars and a cool one cool ones. */
    background: color-mix(in oklch, oklch(var(--tint-bg-l, .4) .09 var(--hue)) 60%, var(--bg3));
    color: oklch(var(--tint-ink-l, .88) .11 var(--hue));
    font-family: var(--display); font-weight: 700; font-size: calc(var(--s) * 0.5);
    overflow: visible;
  }
  img { width: 100%; height: 100%; border-radius: var(--r-avatar, 35%); object-fit: cover; }
  .dot {
    position: absolute; right: -2px; bottom: -2px; border-radius: 50%;
    width: max(8px, calc(var(--s) * .28)); height: max(8px, calc(var(--s) * .28));
    background: var(--lamp); box-shadow: 0 0 0 2px var(--bg-2), 0 0 8px var(--lamp);
  }
</style>
