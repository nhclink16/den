<script lang="ts">
  import { onMount } from 'svelte'
  import { store, instances } from './lib/store.svelte'
  import { router } from './lib/router.svelte'
  import Native from './ui/Native.svelte'
  import Login from './ui/Login.svelte'
  import Shell from './ui/Shell.svelte'
  import Mark from './ui/Mark.svelte'

  let checked = $state(false)

  onMount(async () => {
    document.getElementById('boot')?.remove()
    const ok = await instances.resume()
    checked = true
    if (!ok && router.route.name !== 'login') router.go('/login', true)
    if (ok && router.route.name === 'login') router.go('/', true)
  })
</script>

<Native />
<svelte:window onkeydown={e => instances.key(e)} />

<svelte:head><title>{store.totalUnread && store.me ? `(${store.totalUnread}) ` : ''}{store.settings.instance_name}</title></svelte:head>

<!-- The Den mark in the middle of the screen: where Den Shell's launch screen draws it (apps/ios-shell, LaunchMark)
     and index.html did until now, so opening the app doesn't jump. -->
{#if !checked}
  <div class="boot"><Mark size={64} /><span class="sr-only">{store.settings.instance_name}</span></div>
{:else if router.route.name === 'login' || !store.me}
  <Login />
{:else if store.ready}
  <Shell />
{:else}
  <div class="boot" role="status"><Mark size={64} /><span class="faint">Opening the room</span></div>
{/if}

<style>
  .boot { position: fixed; inset: 0; display: grid; place-items: center; color: var(--ink); }
  /* Under the mark without moving it, and only once the wait is long enough to need words. */
  .boot .faint { position: absolute; top: calc(50% + 48px); font-size: 13px; }
  @media (prefers-reduced-motion: no-preference) { .boot .faint { animation: show .3s 1s both; } }
  @keyframes show { from { opacity: 0; } }
</style>
