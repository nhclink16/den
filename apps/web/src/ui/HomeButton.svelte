<script lang="ts">
  // Back to the room list on narrow screens, with the unread count waiting there.
  // A page opened from the list goes back the way it came, so the list keeps its
  // place; a page reached any other way (a link, a notification, a reload)
  // swaps itself for the list rather than piling one more entry on top.
  // `onback` and `label` make it the same chevron for any other way back (a thread to its room,
  // a Settings page to the list), so every back button sits in the same place.
  import { instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  let { onback, label, aria }: { onback?: () => void; label?: string; aria?: string } = $props()
  const count = $derived(instances.totalUnread)
  function home() {
    if (onback) return onback()
    if (router.parent !== '/') return router.go('/', true)
    // The entry behind is usually the list, but a wide window swaps the list for
    // its first room. If Back lands anywhere else, put the list there instead.
    window.addEventListener('popstate', () => { if (router.route.name !== 'home') router.go('/', true) }, { once: true })
    router.back()
  }
  const text = $derived(label ?? (onback || !count ? '' : count > 99 ? '99+' : String(count)))
</script>

<button class="back" onclick={home} aria-label={aria ?? (label || (count ? `Rooms, ${count} unread` : 'Rooms'))}>
  <Icon name="back" size={24} />{#if text}<span class="label" aria-hidden="true">{text}</span>{/if}
</button>

<style>
  /* The iPhone's back button: a tinted chevron with the count or the previous screen's name beside it, no fill. */
  .back { display: inline-flex; align-items: center; flex: none; min-width: 44px; height: 44px; padding: 0 6px 0 0; margin-inline-start: -4px; color: var(--lamp); }
  .back:active { opacity: .5; }
  .label { margin-inline-start: -3px; font-size: 17px; line-height: 1; font-variant-numeric: tabular-nums; white-space: nowrap; }
</style>
