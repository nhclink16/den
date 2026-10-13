<script lang="ts">
  // Back to the room list on narrow screens, with the unread count waiting there.
  // A page opened from the list goes back the way it came, so the list keeps its
  // place; a page reached any other way (a link, a notification, a reload)
  // swaps itself for the list rather than piling one more entry on top.
  import { instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  const count = $derived(instances.totalUnread)
  function home() {
    if (router.parent !== '/') return router.go('/', true)
    // The entry behind is usually the list, but a wide window swaps the list for
    // its first room. If Back lands anywhere else, put the list there instead.
    window.addEventListener('popstate', () => { if (router.route.name !== 'home') router.go('/', true) }, { once: true })
    router.back()
  }
</script>

<button class="back" onclick={home} aria-label={count ? `Rooms, ${count} unread` : 'Rooms'}>
  <Icon name="back" size={22} />{#if count}<span class="count" aria-hidden="true">{count}</span>{/if}
</button>

<style>
  .back { display: inline-flex; align-items: center; flex: none; min-width: var(--hit); height: var(--hit); padding: 0 6px 0 2px; margin-inline-start: -6px; border-radius: var(--r-pill); color: var(--lamp); }
  .back:active { background: var(--press); }
  .count { min-width: 20px; height: 20px; padding: 0 6px; display: inline-grid; place-items: center; border-radius: var(--r-pill, 999px); background: var(--lamp); color: var(--on-lamp); font-size: 12px; font-weight: 700; line-height: 1; font-variant-numeric: tabular-nums; }
</style>
