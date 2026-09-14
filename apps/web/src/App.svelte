<script lang="ts">
  import { onMount } from 'svelte'
  import { store, instances } from './lib/store.svelte'
  import { router } from './lib/router.svelte'
  import Native from './ui/Native.svelte'
  import Login from './ui/Login.svelte'
  import Shell from './ui/Shell.svelte'

  let checked = $state(false)

  onMount(async () => {
    const ok = await instances.resume()
    checked = true
    if (!ok && router.route.name !== 'login') router.go('/login', true)
    if (ok && router.route.name === 'login') router.go('/', true)
  })

  $effect(() => {
    if (store.ready && router.route.name === 'home') {
      const first = store.textChannels.find((c) => c.kind === 'text') || store.dms[0]
      if (first) router.go(`/c/${first.id}`, true)
    }
  })
</script>

<Native />
<svelte:window onkeydown={e => instances.key(e)} />

<svelte:head><title>{store.totalUnread && store.me ? `(${store.totalUnread}) ` : ''}{store.settings.instance_name}</title></svelte:head>

{#if !checked}
  <div class="boot"><span class="display">{store.settings.instance_name}</span></div>
{:else if router.route.name === 'login' || !store.me}
  <Login />
{:else if store.ready}
  <Shell />
{:else}
  <div class="boot"><span class="display">{store.settings.instance_name}</span><span class="faint">Opening the room</span></div>
{/if}

<style>
  .boot {
    height: 100%; display: grid; place-content: center; gap: 8px; text-align: center;
    font-size: 28px; color: var(--ink-2);
  }
  .boot .faint { font-size: 13px; }
</style>
