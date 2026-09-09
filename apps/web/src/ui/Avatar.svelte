<script lang="ts">
  import { store } from '../lib/store.svelte'
  let { userId, size = 32 }: { userId: string; size?: number } = $props()
  const user = $derived(store.user(userId))
  const label = $derived(user ? (user.display_name || user.username) : '?')
  // Deterministic warm hue per user so avatars are recognizable without images.
  const hue = $derived([...userId].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7))
  const online = $derived(store.online.has(userId))
</script>

<span class="avatar" style="--s:{size}px; --h:{hue}" title={label}>
  {#if user?.avatar_url}
    <img src={user.avatar_url} alt="" />
  {:else}
    <span class="initial">{label.slice(0, 1).toUpperCase()}</span>
  {/if}
  {#if online}<span class="dot" aria-label="online"></span>{/if}
</span>

<style>
  .avatar {
    position: relative; display: inline-grid; place-items: center; flex: none;
    width: var(--s); height: var(--s); border-radius: 35%;
    background: hsl(var(--h) 28% 30%); color: hsl(var(--h) 40% 85%);
    font-family: var(--display); font-weight: 700; font-size: calc(var(--s) * 0.5);
    overflow: visible;
  }
  img { width: 100%; height: 100%; border-radius: 35%; object-fit: cover; }
  .dot {
    position: absolute; right: -2px; bottom: -2px; width: 9px; height: 9px; border-radius: 50%;
    background: var(--lamp); box-shadow: 0 0 0 2px var(--bg-2), 0 0 8px var(--lamp);
  }
</style>
