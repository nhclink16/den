<script lang="ts">
  import { store, type Store } from '../lib/store.svelte'
  import { mediaUrl } from '../lib/native'
  let { userId, size = 32, instance = store }: { userId: string; size?: number; instance?: Store } = $props()
  const user = $derived(instance.user(userId))
  const label = $derived(user ? (user.display_name || user.username) : '?')
  const online = $derived(instance.online.has(userId))
</script>

<span class="avatar" style="--s:{size}px" title={label}>
  {#if user?.avatar_url}
    <img src={mediaUrl(user.avatar_url, instance.origin)} alt="" />
  {:else}
    <span class="initial">{label.slice(0, 1).toUpperCase()}</span>
  {/if}
  {#if online}<span class="dot" aria-label="online"></span>{/if}
</span>

<style>
  .avatar {
    position: relative; display: inline-grid; place-items: center; flex: none;
    width: var(--s); height: var(--s); border-radius: 35%;
    background: var(--bg3); color: var(--ink2);
    font-family: var(--display); font-weight: 700; font-size: calc(var(--s) * 0.5);
    overflow: visible;
  }
  img { width: 100%; height: 100%; border-radius: 35%; object-fit: cover; }
  .dot {
    position: absolute; right: -2px; bottom: -2px; width: 9px; height: 9px; border-radius: 50%;
    background: var(--lamp); box-shadow: 0 0 0 2px var(--bg-2), 0 0 8px var(--lamp);
  }
</style>
